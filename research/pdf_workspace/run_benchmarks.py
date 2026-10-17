"""Run the built experiment against fixtures. No downloads or private PDF discovery."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parent


def command_output(args):
    result = subprocess.run(args, text=True, capture_output=True)
    return result.stdout.strip() if result.returncode == 0 else "unavailable"


def run():
    parser = argparse.ArgumentParser()
    parser.add_argument("--viewport", action="store_true")
    args = parser.parse_args()
    binary = ROOT / "target/release/iqs-pdf-spike"
    library = os.environ.get("IQS_PDFIUM_LIBRARY")
    if not binary.exists() or not library:
        sys.exit("Build the release binary and set IQS_PDFIUM_LIBRARY first; see README.md")
    results = ROOT / "results"
    results.mkdir(exist_ok=True)
    report = {
        "utc_time": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "platform": platform.platform(),
        "arch": platform.machine(),
        "hardware_model": command_output(["sysctl", "-n", "hw.model"]) if sys.platform == "darwin" else "unavailable",
        "cpu": command_output(["sysctl", "-n", "machdep.cpu.brand_string"]) if sys.platform == "darwin" else platform.processor(),
        "memory_bytes": command_output(["sysctl", "-n", "hw.memsize"]) if sys.platform == "darwin" else "unavailable",
        "rust": command_output(["rustc", "--version"]),
        "build": "release; Cargo.lock; single sequential run; OS/font caches uncontrolled",
        "library_sha256": hashlib.sha256(Path(library).read_bytes()).hexdigest(),
        "cases": [],
    }
    for fixture in json.loads((ROOT / "fixtures/manifest.json").read_text()):
        path = ROOT / "fixtures" / fixture["file"]
        assert hashlib.sha256(path.read_bytes()).hexdigest() == fixture["sha256"], path
        env = dict(os.environ)
        env.pop("IQS_RENDER_PPM", None)
        cmd = [str(binary), "--benchmark", str(path)]
        if sys.platform == "darwin":
            cmd = ["/usr/bin/time", "-l"] + cmd
        result = subprocess.run(cmd, env=env, cwd=ROOT, text=True, capture_output=True, timeout=90)
        (results / (path.stem + ".json")).write_text(result.stdout)
        (results / (path.stem + "-memory.txt")).write_text(result.stderr)
        case = {"fixture": fixture["file"], "sha256": fixture["sha256"], "exit_code": result.returncode,
                "expected": fixture["expected"]}
        if fixture["expected"] == "error":
            assert result.returncode != 0 and "Error:" in result.stderr, result.stderr
            case["error"] = result.stderr.splitlines()[0]
        else:
            assert result.returncode == 0, result.stderr
            case["measurement"] = json.loads(result.stdout)
        rss = re.search(r"(\d+)\s+maximum resident set size", result.stderr)
        case["max_rss_bytes"] = int(rss[1]) if rss else None
        report["cases"].append(case)
    parity = subprocess.run([os.environ.get("IQS_NODE", "node"), str(ROOT / "legacy_parity.mjs")], text=True, capture_output=True, timeout=45)
    assert parity.returncode == 0, parity.stderr
    report["legacy"] = json.loads(parity.stdout)
    if args.viewport:
        env = dict(os.environ, IQS_SCREENSHOT_PPM=str(results / "viewport.ppm"))
        cmd = [str(binary), "--smoke", str(ROOT / "fixtures/vector.pdf")]
        if sys.platform == "darwin":
            cmd = ["/usr/bin/time", "-l"] + cmd
        result = subprocess.run(cmd, env=env, cwd=ROOT, text=True, capture_output=True, timeout=45)
        (results / "viewport.json").write_text(result.stdout)
        (results / "viewport-memory.txt").write_text(result.stderr)
        assert result.returncode == 0, result.stderr
        report["viewport"] = json.loads(result.stdout)
        rss = re.search(r"(\d+)\s+maximum resident set size", result.stderr)
        report["viewport"]["max_rss_bytes"] = int(rss[1]) if rss else None
    output = results / "report.json"
    output.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Verified {len(report['cases'])} PDF cases and {report['legacy']['cases']} quantity cases; report: {output}")


if __name__ == "__main__":
    run()
