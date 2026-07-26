#!/usr/bin/env node

/**
 * Development-only fixed-view renderer for fast visual iteration.
 *
 * It renders an already compiled GLB in a temporary local page with Three.js.
 * The output is not product state, formal evidence, or a substitute for the
 * one-renderer workbench Gate.  Its only purpose is to shorten the
 * Recipe -> GLB -> pixels feedback loop while an asset is still draft.
 */

import { createServer } from 'node:http'
import { existsSync } from 'node:fs'
import { mkdir, readFile, realpath } from 'node:fs/promises'
import { extname, resolve } from 'node:path'
import process from 'node:process'
import { chromium } from 'playwright-core'

const ROOT = resolve(import.meta.dirname, '..')
const THREE_ROOT = resolve(ROOT, 'node_modules/three')
const MODULES = new Map([
  ['/three.module.js', resolve(THREE_ROOT, 'build/three.module.js')],
  ['/three.core.js', resolve(THREE_ROOT, 'build/three.core.js')],
  ['/GLTFLoader.js', resolve(THREE_ROOT, 'examples/jsm/loaders/GLTFLoader.js')],
  ['/RoomEnvironment.js', resolve(THREE_ROOT, 'examples/jsm/environments/RoomEnvironment.js')],
  ['/utils/BufferGeometryUtils.js', resolve(THREE_ROOT, 'examples/jsm/utils/BufferGeometryUtils.js')],
  ['/utils/SkeletonUtils.js', resolve(THREE_ROOT, 'examples/jsm/utils/SkeletonUtils.js')],
])
const VIEW_IDS = new Set(['iso', 'front', 'back', 'left', 'right', 'top', 'gripper_iso', 'gripper_front', 'link_iso', 'link_side', 'base_iso', 'base_front'])
const CHROME_CANDIDATES = [
  process.env.FORGECAD_CHROME_EXECUTABLE,
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/Applications/Chromium.app/Contents/MacOS/Chromium',
  '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
].filter(Boolean)

function fail(message) {
  throw new Error(message)
}

function parseArgs(argv) {
  const args = { glb: null, output: null, views: ['iso', 'front', 'right'] }
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index]
    const value = argv[index + 1]
    if (key === '--glb' || key === '--output' || key === '--views') {
      if (!value || value.startsWith('--')) fail(`${key} requires a value`)
      if (key === '--glb') args.glb = value
      if (key === '--output') args.output = value
      if (key === '--views') args.views = value.split(',').filter(Boolean)
      index += 1
    } else {
      fail(`unknown argument: ${key}`)
    }
  }
  if (!args.glb || !args.output) fail('--glb and --output are required')
  if (args.views.length === 0 || args.views.some((view) => !VIEW_IDS.has(view))) {
    fail('views must be a comma-separated subset of iso,front,back,left,right,top,gripper_iso,gripper_front,link_iso,link_side,base_iso,base_front')
  }
  return args
}

function pageSource() {
  return `<!doctype html>
<html><head><meta charset="utf-8"><style>
html,body{margin:0;width:100%;height:100%;overflow:hidden;background:#09111c}canvas{display:block}
#label{position:fixed;left:18px;top:16px;padding:8px 12px;border:1px solid #243246;border-radius:8px;color:#c8d5e6;background:#08111dcc;font:14px ui-monospace,monospace;z-index:2}
</style><script type="importmap">{"imports":{"three":"/three.module.js"}}</script></head>
<body><div id="label">development visual iteration</div><script type="module">
import * as THREE from 'three'
import { GLTFLoader } from '/GLTFLoader.js'
import { RoomEnvironment } from '/RoomEnvironment.js'

const renderer = new THREE.WebGLRenderer({antialias:true,preserveDrawingBuffer:true})
renderer.setPixelRatio(1)
renderer.setSize(innerWidth,innerHeight)
renderer.outputColorSpace=THREE.SRGBColorSpace
renderer.toneMapping=THREE.ACESFilmicToneMapping
renderer.toneMappingExposure=.62
renderer.shadowMap.enabled=true
renderer.shadowMap.type=THREE.PCFShadowMap
document.body.append(renderer.domElement)
const scene=new THREE.Scene(); scene.background=new THREE.Color('#09111c')
const camera=new THREE.PerspectiveCamera(36,innerWidth/innerHeight,.001,1000)
const pmrem=new THREE.PMREMGenerator(renderer)
const env=new RoomEnvironment(); scene.environment=pmrem.fromScene(env,.04).texture; env.dispose(); pmrem.dispose()
const hemisphere=new THREE.HemisphereLight('#dbeaff','#0a1119',.58); scene.add(hemisphere)
const ambient=new THREE.AmbientLight('#8aa0b8',.12); scene.add(ambient)
const key=new THREE.DirectionalLight('#f5f9ff',1.35); key.position.set(3.2,4.8,4.2); key.castShadow=true; scene.add(key)
const rim=new THREE.DirectionalLight('#4d8ed8',.5); rim.position.set(-4,2,-3); scene.add(rim)
const warm=new THREE.DirectionalLight('#ffc6a8',.1); warm.position.set(3,.5,-4); scene.add(warm)
const intensity={mat_primary:.38,mat_aluminum:.26,mat_composite:.18,mat_rubber:.1,mat_dark_glass:.28,mat_emissive_blue:.14,mat_automotive_paint:.32}

const gltf=await new GLTFLoader().loadAsync('/model.glb')
const environment=gltf.parser?.json?.extras?.forgecad_visual_environment
if(!environment||!['env_forgecad_room_studio_v1','env_forgecad_room_studio_v2'].includes(environment.environment_id))throw new Error('GLB visual environment contract missing or unsupported')
const lighting=environment.cad_neutral_lighting
renderer.toneMappingExposure=environment.tone_mapping_exposure
scene.background.set(lighting.background)
hemisphere.color.set(lighting.hemisphere.sky);hemisphere.groundColor.set(lighting.hemisphere.ground);hemisphere.intensity=lighting.hemisphere.intensity
ambient.color.set(lighting.ambient.color);ambient.intensity=lighting.ambient.intensity
key.color.set(lighting.key.color);key.intensity=lighting.key.intensity;key.position.fromArray(lighting.key.position)
rim.color.set(lighting.rim.color);rim.intensity=lighting.rim.intensity;rim.position.fromArray(lighting.rim.position)
warm.color.set(lighting.warm_rim.color);warm.intensity=lighting.warm_rim.intensity;warm.position.fromArray(lighting.warm_rim.position)
const model=gltf.scene
model.traverse((object)=>{if(!object.isMesh)return;object.castShadow=true;object.receiveShadow=true;const materials=Array.isArray(object.material)?object.material:[object.material];for(const material of materials){if(!material.isMeshStandardMaterial&&!material.isMeshPhysicalMaterial)continue;const id=material.userData?.forgecad_texture_material_id||'';material.envMapIntensity=intensity[id]??.3;material.needsUpdate=true}})
scene.add(model)
const box=new THREE.Box3().setFromObject(model), size=box.getSize(new THREE.Vector3()), center=box.getCenter(new THREE.Vector3())
model.position.sub(center)
const floorY=-size.y/2
const floor=new THREE.Mesh(new THREE.CircleGeometry(Math.max(size.x,size.z)*.72,96),new THREE.ShadowMaterial({color:'#000',opacity:.28,transparent:true}))
floor.rotation.x=-Math.PI/2; floor.position.y=floorY-.002; floor.receiveShadow=true; scene.add(floor)
const sphere=new THREE.Sphere(); new THREE.Box3().setFromObject(model).getBoundingSphere(sphere)
const directions={iso:new THREE.Vector3(-.9,.78,1.45),front:new THREE.Vector3(0,0,1),back:new THREE.Vector3(0,0,-1),left:new THREE.Vector3(-1,0,0),right:new THREE.Vector3(1,0,0),top:new THREE.Vector3(0,1,0),gripper_iso:new THREE.Vector3(-.9,.78,1.45),gripper_front:new THREE.Vector3(0,0,1),link_iso:new THREE.Vector3(-.75,.55,1.35),link_side:new THREE.Vector3(0,0,1),base_iso:new THREE.Vector3(-.9,.72,1.45),base_front:new THREE.Vector3(0,0,1)}
window.renderView=(view)=>{const gripper=view.startsWith('gripper_'),link=view.startsWith('link_'),base=view.startsWith('base_'),target=gripper?new THREE.Vector3(-.78,.13,0):link?new THREE.Vector3(.05,.28,0):base?new THREE.Vector3(.62,-.68,0):new THREE.Vector3(),radius=gripper?.24:link?.38:base?.46:sphere.radius,direction=directions[view].clone().normalize(),distance=radius/Math.sin(THREE.MathUtils.degToRad(camera.fov*.5))*(gripper?1.18:link?1.04:base?1.08:.88);camera.up.set(0,view==='top'?0:1,view==='top'?-1:0);camera.position.copy(target).add(direction.multiplyScalar(distance));camera.lookAt(target);camera.near=Math.max(distance-radius*1.6,.001);camera.far=distance+radius*2.2;camera.updateProjectionMatrix();document.querySelector('#label').textContent=view+' / development only';renderer.render(scene,camera)}
window.__forgecadVisualIterationReady=true
</script></body></html>`
}

async function main() {
  const args = parseArgs(process.argv.slice(2))
  const glbPath = await realpath(resolve(args.glb))
  if (extname(glbPath).toLowerCase() !== '.glb') fail('--glb must point to a GLB file')
  const glbBytes = await readFile(glbPath)
  const output = resolve(args.output)
  await mkdir(output, { recursive: true })
  const server = createServer(async (request, response) => {
    try {
      if (request.url === '/') {
        response.writeHead(200, {'content-type':'text/html; charset=utf-8'})
        response.end(pageSource())
      } else if (request.url === '/favicon.ico') {
        response.writeHead(204); response.end()
      } else if (request.url === '/model.glb') {
        response.writeHead(200, {'content-type':'model/gltf-binary','content-length':glbBytes.length})
        response.end(glbBytes)
      } else if (MODULES.has(request.url)) {
        const bytes = await readFile(MODULES.get(request.url))
        response.writeHead(200, {'content-type':'text/javascript; charset=utf-8','content-length':bytes.length})
        response.end(bytes)
      } else {
        response.writeHead(404); response.end()
      }
    } catch (error) {
      response.writeHead(500); response.end(String(error))
    }
  })
  await new Promise((resolveListen, rejectListen) => {
    server.once('error', rejectListen)
    server.listen(0, '127.0.0.1', resolveListen)
  })
  const address = server.address()
  if (!address || typeof address === 'string') fail('local renderer address unavailable')
  const executablePath = CHROME_CANDIDATES.find((candidate) => existsSync(candidate))
  const browser = await chromium.launch({ headless: true, ...(executablePath ? { executablePath } : {}) })
  try {
    const page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 })
    page.on('console', (message) => process.stderr.write(`[browser:${message.type()}] ${message.text()}\n`))
    page.on('pageerror', (error) => process.stderr.write(`[browser:error] ${error.message}\n`))
    page.on('response', (response) => {
      if (response.status() >= 400) process.stderr.write(`[browser:http] ${response.status()} ${response.url()}\n`)
    })
    await page.goto(`http://127.0.0.1:${address.port}/`, { waitUntil: 'networkidle' })
    await page.waitForFunction(() => window.__forgecadVisualIterationReady === true)
    for (const view of args.views) {
      await page.evaluate((viewId) => window.renderView(viewId), view)
      await page.screenshot({ path: resolve(output, `${view}.png`) })
    }
    process.stdout.write(`${JSON.stringify({schema_version:'ForgeCADVisualIterationCapture@1',formal_eligible:false,glb:glbPath,views:args.views,output})}\n`)
  } finally {
    await browser.close()
    await new Promise((resolveClose) => server.close(resolveClose))
  }
}

main().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.stack : String(error)}\n`)
  process.exitCode = 1
})
