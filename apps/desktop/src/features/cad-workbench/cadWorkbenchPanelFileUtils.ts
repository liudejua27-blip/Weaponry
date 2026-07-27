export function downloadBase64File(encoded: string, filename: string, mime: string): void {
  const bytes = Uint8Array.from(window.atob(encoded), (character) => character.charCodeAt(0))
  downloadBlobFile(new Blob([bytes], { type: mime }), filename)
}

export function downloadBlobFile(blob: Blob, filename: string): void {
  const objectUrl = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = objectUrl
  anchor.download = filename
  anchor.style.display = 'none'
  document.body.appendChild(anchor)
  anchor.click()
  anchor.remove()
  window.setTimeout(() => URL.revokeObjectURL(objectUrl), 0)
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
