"""Local M1a checks; optional read-only project PDF, never copied to fixtures."""
import argparse
import datetime
import json
import os
from pathlib import Path
import platform
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent


def children(pid):
    if sys.platform not in ("darwin", "linux"):
        return []
    result = subprocess.run(["pgrep", "-P", str(pid)], capture_output=True, text=True)
    return [int(p) for p in result.stdout.split() if p.isdigit()]


def rss(pids):
    if not pids or sys.platform not in ("darwin", "linux"):
        return {}
    result = subprocess.run(["ps", "-o", "pid=,rss=", "-p", ",".join(map(str, pids))], capture_output=True, text=True)
    values = {}
    for line in result.stdout.splitlines():
        parts = line.split()
        if len(parts) == 2 and all(p.isdigit() for p in parts):
            values[int(parts[0])] = int(parts[1]) * 1024
    return values


def check(name, arguments, *, screenshot=False):
    env = dict(os.environ)
    if screenshot:
        env["IQS_SCREENSHOT_PPM"] = str(ROOT / "results" / (name + ".ppm"))
    process = subprocess.Popen([str(ROOT / "target/release/iqs-pdf-spike")] + arguments,
                               cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    peaks = {"parent_peak_bytes": 0, "workers_peak_bytes": 0, "combined_sampled_peak_bytes": 0}
    started = time.monotonic()
    while process.poll() is None:
        workers = children(process.pid)
        values = rss([process.pid] + workers)
        parent = values.get(process.pid, 0)
        worker = sum(values.get(pid, 0) for pid in workers)
        peaks["parent_peak_bytes"] = max(peaks["parent_peak_bytes"], parent)
        peaks["workers_peak_bytes"] = max(peaks["workers_peak_bytes"], worker)
        peaks["combined_sampled_peak_bytes"] = max(peaks["combined_sampled_peak_bytes"], parent + worker)
        if time.monotonic() - started > 120:
            for pid in children(process.pid):
                try:
                    os.kill(pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            process.kill()
            raise RuntimeError(f"{name}: exceeded 120-second test deadline")
        time.sleep(0.1)
    output, errors = process.communicate()
    (ROOT / "results" / (name + ".json")).write_text(output)
    (ROOT / "results" / (name + ".log")).write_text(errors)
    if process.returncode:
        raise RuntimeError(f"{name}: {errors.strip()}")
    data = json.loads(output)
    if not data.get("success"):
        raise RuntimeError(f"{name}: validation reported failure")
    data["memory_sampling"] = dict(peaks, interval_ms=100,
        method="ps RSS for this test process and its direct worker children; sampled peaks can miss spikes; excludes GPU driver allocations")
    return data


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--project-pdf", type=Path)
    args = parser.parse_args()
    if not os.environ.get("IQS_PDFIUM_LIBRARY"):
        sys.exit("Set IQS_PDFIUM_LIBRARY; see README.md")
    (ROOT / "results").mkdir(exist_ok=True)
    results = {"utc_time": datetime.datetime.now(datetime.timezone.utc).isoformat(),
               "platform": platform.platform(), "build": "release/Cargo.lock", "checks": {}}
    results["checks"]["pipeline"] = check("pipeline", ["--pipeline-test", str(ROOT / "fixtures")])
    results["checks"]["dense_viewport"] = check("progressive-dense", ["--smoke", str(ROOT / "fixtures/dense.pdf")], screenshot=True)
    if args.project_pdf:
        source = args.project_pdf.resolve(strict=True)
        results["checks"]["project_samples"] = check("project-sample", ["--document-test", str(source)])
        slowest=max(results["checks"]["project_samples"]["samples"],key=lambda s:s["preview_render_ms"])["page"]
        results["checks"]["project_viewport"] = check("progressive-project", ["--smoke", str(source), "--page", str(slowest)], screenshot=True)
    path = ROOT / "results/progressive-report.json"
    path.write_text(json.dumps(results, indent=2) + "\n")
    print(f"Passed {len(results['checks'])} progressive checks; report: {path}")


if __name__ == "__main__":
    main()
