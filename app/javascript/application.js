// Configure your import map in config/importmap.rb. Read more: https://github.com/rails/importmap-rails
import "@hotwired/turbo-rails"
import "controllers"
import * as bootstrap from "bootstrap"
import * as ActiveStorage from "@rails/activestorage"

ActiveStorage.start()

document.addEventListener("direct-uploads:start", (event) => {
  const form = event.target
  form.querySelectorAll("[data-direct-upload-status]").forEach((status) => {
    status.textContent = "Preparing upload..."
  })
})

document.addEventListener("direct-upload:progress", (event) => {
  const input = event.target
  const status = input.closest("form")?.querySelector("[data-direct-upload-status]")
  if (!status) return

  const { file, progress } = event.detail
  status.textContent = `${file.name}: ${Math.round(progress)}% uploaded`
})

document.addEventListener("direct-upload:error", (event) => {
  const input = event.target
  const status = input.closest("form")?.querySelector("[data-direct-upload-status]")
  if (!status) return

  event.preventDefault()
  status.textContent = event.detail.error
  status.classList.add("text-danger")
})

document.addEventListener("direct-uploads:end", (event) => {
  const form = event.target
  form.querySelectorAll("[data-direct-upload-status]").forEach((status) => {
    status.textContent = "Upload complete. Saving document details..."
    status.classList.remove("text-danger")
  })
})

document.addEventListener("click", (event) => {
  const button = event.target.closest("[data-required-by-offset]")
  if (!button) return

  const form = button.closest("form")
  const input = form?.querySelector("[data-required-by-input]")
  if (!input) return

  const date = new Date()
  date.setDate(date.getDate() + Number(button.dataset.requiredByOffset))
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, "0")
  const day = String(date.getDate()).padStart(2, "0")
  input.value = `${year}-${month}-${day}`
})
