export function downloadBase64File(encoded: string, filename: string, mime: string): void {
  const bytes = Uint8Array.from(window.atob(encoded), (character) => character.charCodeAt(0))
  downloadBlobFile(new Blob([bytes], { type: mime }), filename)
}

export function downloadBlobFile(blob: Blob, filename: string): void {
  const objectUrl = URL.createObjectURL(blob)
  downloadUrlFile(objectUrl, filename)
  window.setTimeout(() => URL.revokeObjectURL(objectUrl), 0)
}

export function downloadUrlFile(url: string, filename: string): void {
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = filename
  if (!url.startsWith('blob:')) {
    // Browser development serves read-only resources from a separate
    // loopback origin, where the HTML download attribute may be ignored.
    // Keep that compatibility download from replacing the workbench page.
    anchor.target = '_blank'
    anchor.rel = 'noopener'
  }
  anchor.style.display = 'none'
  document.body.appendChild(anchor)
  anchor.click()
  anchor.remove()
}

export function arrayBufferToBase64(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer)
  const chunkSize = 0x8000
  let binary = ''
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, Math.min(offset + chunkSize, bytes.length)))
  }
  return window.btoa(binary)
}
