import * as THREE from 'three'

import dragonfangProgram from '../../../skills/weaponry-threejs-knife-studio/references/dragonfang-first-slice.json'
import dragonfangObjectiveLedger from '../../../skills/weaponry-threejs-knife-studio/references/dragonfang-objective-ledger-r5.json'
import {
  ThreeAssetStudioController,
  type ThreeAssetStudioActionName,
  type ThreeAssetStudioActionResult,
} from '../src/three-asset-studio.ts'
import type { KnifeSceneProgram } from '../src/knife-scene-program.ts'

const canvas = document.querySelector<HTMLCanvasElement>('#studio-canvas')
const status = document.querySelector<HTMLDivElement>('#status')
const receipt = document.querySelector<HTMLPreElement>('#receipt')
if (!canvas || !status || !receipt) throw new Error('studio host elements are missing')

const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, preserveDrawingBuffer: true })
renderer.setPixelRatio(1)
renderer.setSize(256, 256, false)
renderer.outputColorSpace = THREE.SRGBColorSpace
renderer.setClearColor('#080b10', 1)

const controller = new ThreeAssetStudioController()
let designId: string | undefined
let candidateId: string | undefined
let busy = false

for (const button of document.querySelectorAll<HTMLButtonElement>('[data-action]')) {
  button.addEventListener('click', () => {
    const action = button.dataset.action
    if (!isActionName(action) || busy) return
    void runAction(action)
  })
}

async function runAction(action: ThreeAssetStudioActionName): Promise<void> {
  busy = true
  setButtonsDisabled(true)
  status.textContent = `${action} · running bounded in-process action`
  try {
    const result = await dispatchAction(action)
    if (result.action === 'knife_design_create') designId = result.design_id
    if (result.action === 'optimize') candidateId = result.selected_candidate_id
    if (result.action === 'three_asset_build') candidateId = result.candidate_id
    if (result.action === 'candidates_generate') candidateId = undefined
    receipt.textContent = JSON.stringify(displayResult(result), null, 2)
    status.textContent = `${result.status} · quality NOT_RUN · Runtime/Store/MCP untouched`
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    receipt.textContent = JSON.stringify({ action, status: 'ERROR', message }, null, 2)
    status.textContent = `blocked · ${message}`
  } finally {
    busy = false
    setButtonsDisabled(false)
  }
}

async function dispatchAction(action: ThreeAssetStudioActionName): Promise<ThreeAssetStudioActionResult> {
  if (action === 'knife_design_create') {
    return controller.dispatch({
      action,
      request_id: `studio-${action}`,
      program: dragonfangProgram as unknown as KnifeSceneProgram,
    })
  }
  if (action === 'candidates_generate') {
    const ensuredDesignId = await ensureDesign()
    return controller.dispatch({
      action,
      request_id: `studio-${action}`,
      design_id: ensuredDesignId,
      objective_ledger: dragonfangObjectiveLedger,
      candidate_count: 3,
    })
  }
  if (action === 'optimize') {
    const ensuredDesignId = await ensureDesign()
    await ensureCandidates(ensuredDesignId)
    return controller.dispatch({ action, request_id: `studio-${action}`, design_id: ensuredDesignId })
  }
  if (action === 'three_asset_build') {
    const ensuredCandidateId = await ensureCandidate()
    return controller.dispatch({ action, request_id: `studio-${action}`, candidate_id: ensuredCandidateId })
  }
  if (action === 'preview') {
    const ensuredCandidateId = await ensureCandidate()
    return controller.dispatch({
      action,
      request_id: `studio-${action}`,
      candidate_id: ensuredCandidateId,
      view_ids: ['FRONT'],
      capture_aovs: true,
    }, { renderer })
  }
  const ensuredCandidateId = await ensureCandidate()
  return controller.dispatch({ action, request_id: `studio-${action}`, candidate_id: ensuredCandidateId })
}

async function ensureDesign(): Promise<string> {
  if (designId) return designId
  const result = await controller.dispatch({
    action: 'knife_design_create',
    request_id: 'studio-auto-design-create',
    program: dragonfangProgram as unknown as KnifeSceneProgram,
  })
  if (result.action !== 'knife_design_create') throw new Error('design action returned an unexpected result')
  designId = result.design_id
  return designId
}

async function ensureCandidates(ensuredDesignId: string): Promise<void> {
  const result = await controller.dispatch({
    action: 'candidates_generate',
    request_id: 'studio-auto-candidates-generate',
    design_id: ensuredDesignId,
    objective_ledger: dragonfangObjectiveLedger,
    candidate_count: 3,
  })
  if (result.action !== 'candidates_generate' || !result.candidates[0]) throw new Error('candidate action returned no bounded result')
}

async function ensureCandidate(): Promise<string> {
  if (candidateId) {
    await ensureBuilt(candidateId)
    return candidateId
  }
  const ensuredDesignId = await ensureDesign()
  await ensureCandidates(ensuredDesignId)
  const optimized = await controller.dispatch({
    action: 'optimize',
    request_id: 'studio-auto-optimize',
    design_id: ensuredDesignId,
  })
  if (optimized.action !== 'optimize') throw new Error('optimize action returned an unexpected result')
  candidateId = optimized.selected_candidate_id
  await ensureBuilt(candidateId)
  return candidateId
}

async function ensureBuilt(ensuredCandidateId: string): Promise<void> {
  await controller.dispatch({
    action: 'three_asset_build',
    request_id: 'studio-auto-three-asset-build',
    candidate_id: ensuredCandidateId,
  })
}

function displayResult(result: ThreeAssetStudioActionResult): unknown {
  if (result.action !== 'export') return result
  const { glb_base64: _glbBase64, ...summary } = result
  return { ...summary, glb_payload: 'held in action result; not downloaded or written' }
}

function isActionName(value: string | undefined): value is ThreeAssetStudioActionName {
  return value === 'knife_design_create'
    || value === 'candidates_generate'
    || value === 'optimize'
    || value === 'three_asset_build'
    || value === 'preview'
    || value === 'export'
}

function setButtonsDisabled(disabled: boolean): void {
  for (const button of document.querySelectorAll<HTMLButtonElement>('[data-action]')) button.disabled = disabled
}
