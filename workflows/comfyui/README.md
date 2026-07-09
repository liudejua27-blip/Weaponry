# ComfyUI Workflows

This folder stores API-format ComfyUI workflow templates for Wushen Forge.

The default concept template is:

```text
workflows/comfyui/concept_api_template.json
```

The default patch/inpaint template is:

```text
workflows/comfyui/patch_inpaint_api_template.json
```

It is a minimal Stable Diffusion-style graph intended to prove the provider boundary. For production art direction, export an API workflow from ComfyUI and either replace this template or point the Agent to another template:

```text
WUSHEN_COMFYUI_WORKFLOW_TEMPLATE=/absolute/path/to/exported_api_workflow_template.json
WUSHEN_COMFYUI_PATCH_WORKFLOW_TEMPLATE=/absolute/path/to/exported_patch_api_workflow_template.json
```

Template files must contain:

- `template_id`
- `template_version`
- `prompt`
- `bindings`

The `prompt` value is the exact API prompt graph submitted to ComfyUI. The `bindings` map tells Wushen Forge where to inject prompt, negative prompt, seed, image size, checkpoint, filename prefix, and for patch workflows the uploaded source image and mask image filenames.

Useful overrides:

```text
WUSHEN_COMFYUI_CHECKPOINT=<checkpoint name installed in ComfyUI>
WUSHEN_COMFYUI_WIDTH=1280
WUSHEN_COMFYUI_HEIGHT=720
```

Run the fake-server adapter gate:

```text
npm run agent:m3-comfyui-smoke
npm run agent:m4-comfyui-patch-smoke
```

Run the real local ComfyUI manual smoke:

```text
WUSHEN_IMAGE_PROVIDER=comfyui \
WUSHEN_COMFYUI_BASE_URL=http://127.0.0.1:8188 \
WUSHEN_COMFYUI_CHECKPOINT=<checkpoint name> \
npm run agent:m3-comfyui-manual
```
