type CompareWorkerRequest = {
  id: string
  kind: 'difference' | 'contour'
  width: number
  height: number
  referenceBuffer: ArrayBuffer
  renderBuffer?: ArrayBuffer
  sensitivity?: number
}

type CompareWorkerResponse = {
  id: string
  ok: boolean
  width: number
  height: number
  buffer?: ArrayBuffer
  error?: string
}

const workerScope = self as unknown as {
  onmessage: ((event: MessageEvent<CompareWorkerRequest>) => void) | null
  postMessage: (message: CompareWorkerResponse, transfer?: Transferable[]) => void
}

function createDifferenceImage(
  reference: Uint8ClampedArray,
  render: Uint8ClampedArray,
  sensitivity: number,
): Uint8ClampedArray {
  const output = new Uint8ClampedArray(reference.length)
  for (let index = 0; index < output.length; index += 4) {
    const redDelta = Math.abs(reference[index] - render[index])
    const greenDelta = Math.abs(reference[index + 1] - render[index + 1])
    const blueDelta = Math.abs(reference[index + 2] - render[index + 2])
    const delta = Math.min(1, ((redDelta + greenDelta + blueDelta) / (255 * 3)) * sensitivity)
    const hue = (1 - delta) * 220
    const chroma = 1 - Math.abs((hue / 60) % 2 - 1)
    const sector = Math.floor(hue / 60)
    const base = delta * 255
    const secondary = chroma * base
    const channels = sector === 0 ? [base, secondary, 0] : sector === 1 ? [secondary, base, 0] : sector === 2 ? [0, base, secondary] : sector === 3 ? [0, secondary, base] : sector === 4 ? [secondary, 0, base] : [base, 0, secondary]
    output[index] = channels[0] ?? 0
    output[index + 1] = channels[1] ?? 0
    output[index + 2] = channels[2] ?? 0
    output[index + 3] = delta === 0 ? 0 : Math.round(80 + delta * 150)
  }
  return output
}

function createContourImage(reference: Uint8ClampedArray, width: number, height: number): Uint8ClampedArray {
  const pixelCount = width * height
  const background = new Uint8Array(pixelCount)
  const queue = new Int32Array(pixelCount)
  let queueHead = 0
  let queueTail = 0
  const enqueue = (index: number) => {
    if (background[index] !== 0) return
    background[index] = 1
    queue[queueTail] = index
    queueTail += 1
  }
  for (let offset = 0; offset < width; offset += 1) {
    enqueue(offset)
    enqueue((height - 1) * width + offset)
  }
  for (let offset = 0; offset < height; offset += 1) {
    enqueue(offset * width)
    enqueue(offset * width + width - 1)
  }
  const localBackgroundEdgeThreshold = 18
  while (queueHead < queueTail) {
    const index = queue[queueHead] ?? 0
    queueHead += 1
    const x = index % width
    const y = Math.floor(index / width)
    const currentOffset = index * 4
    for (let direction = 0; direction < 4; direction += 1) {
      const neighbor = direction === 0
        ? (x > 0 ? index - 1 : -1)
        : direction === 1
          ? (x + 1 < width ? index + 1 : -1)
          : direction === 2
            ? (y > 0 ? index - width : -1)
            : (y + 1 < height ? index + width : -1)
      if (neighbor < 0 || background[neighbor] !== 0) continue
      const nextOffset = neighbor * 4
      const distance = Math.abs(reference[currentOffset] - reference[nextOffset])
        + Math.abs(reference[currentOffset + 1] - reference[nextOffset + 1])
        + Math.abs(reference[currentOffset + 2] - reference[nextOffset + 2])
      if (distance <= localBackgroundEdgeThreshold) enqueue(neighbor)
    }
  }
  const foreground = new Uint8Array(pixelCount)
  let foregroundCount = 0
  for (let index = 0; index < pixelCount; index += 1) {
    if (background[index] === 0) {
      foreground[index] = 1
      foregroundCount += 1
    }
  }
  if (foregroundCount === 0) {
    for (let index = 0; index < pixelCount; index += 1) {
      const offset = index * 4
      const luminance = reference[offset] * 0.2126 + reference[offset + 1] * 0.7152 + reference[offset + 2] * 0.0722
      foreground[index] = luminance > 48 ? 1 : 0
    }
  }
  const output = new Uint8ClampedArray(reference.length)
  for (let y = 1; y < height - 1; y += 1) {
    for (let x = 1; x < width - 1; x += 1) {
      const index = y * width + x
      if (!foreground[index]) continue
      if (foreground[index - 1] && foreground[index + 1] && foreground[index - width] && foreground[index + width]) continue
      const offset = index * 4
      output[offset] = 255
      output[offset + 1] = 170
      output[offset + 2] = 55
      output[offset + 3] = 230
    }
  }
  return output
}

workerScope.onmessage = (event) => {
  const request = event.data
  try {
    const reference = new Uint8ClampedArray(request.referenceBuffer)
    const output = request.kind === 'difference'
      ? createDifferenceImage(reference, new Uint8ClampedArray(request.renderBuffer ?? new ArrayBuffer(reference.byteLength)), request.sensitivity ?? 1)
      : createContourImage(reference, request.width, request.height)
    workerScope.postMessage({ id: request.id, ok: true, width: request.width, height: request.height, buffer: output.buffer as ArrayBuffer }, [output.buffer])
  } catch (error) {
    workerScope.postMessage({ id: request.id, ok: false, width: request.width, height: request.height, error: error instanceof Error ? error.message : 'COMPARE_WORKER_FAILED' })
  }
}

export {}
