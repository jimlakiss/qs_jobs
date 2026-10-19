import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["mode", "start", "due", "duration"]

  connect() {
    this.refresh()
  }

  refresh() {
    const fromStart = this.modeTarget.value === "start_date"
    this.startTarget.disabled = !fromStart
    this.startTarget.required = fromStart
    this.dueTarget.disabled = fromStart
    this.durationTarget.required = fromStart
  }
}
