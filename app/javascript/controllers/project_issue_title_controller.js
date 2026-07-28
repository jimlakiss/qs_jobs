import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["preview", "purpose", "revision", "version"]
  static values = { prefix: String }

  connect() {
    this.update()
  }

  update() {
    const purpose = this.cleanValue(this.purposeTarget.value) || "<purpose>"
    const revision = this.cleanValue(this.revisionTarget.value)
    const version = this.cleanValue(this.versionTarget.value)
    const revisionVersion = [
      revision ? `Revision ${revision}` : null,
      version ? `Version ${version}` : null
    ].filter(Boolean).join(" ")

    this.previewTarget.value = [this.prefixValue, purpose, revisionVersion].filter(Boolean).join(" | ")
  }

  cleanValue(value) {
    return value.trim().replace(/\s+/g, " ")
  }
}
