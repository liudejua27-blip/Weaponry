from __future__ import annotations

import json
import os
import urllib.error
import urllib.request
from dataclasses import dataclass
from typing import Any, Dict, Optional, Protocol

from ..models import CreateWeaponRequest, ProviderSettings, utc_now
from forgecad_agent.application.concept_planner import concept_planner_from_env


class LLMProviderError(Exception):
    def __init__(self, code: str, message: str, recoverable: bool = True) -> None:
        super().__init__(message)
        self.code = code
        self.recoverable = recoverable


class LLMProvider(Protocol):
    provider_id: str

    def plan_weapon_spec(self, request: CreateWeaponRequest, *, weapon_id: str) -> Dict[str, Any]:
        ...


@dataclass(frozen=True)
class OpenAICompatibleConfig:
    base_url: str
    model: str
    api_key: Optional[str]
    timeout_seconds: float = 60


class MockLLMProvider:
    provider_id = "mock_llm"

    def plan_weapon_spec(self, request: CreateWeaponRequest, *, weapon_id: str) -> Dict[str, Any]:
        display_name = derive_display_name(request.text)
        weapon_family = derive_weapon_family(request.text)
        return build_fallback_weapon_spec(
            request,
            weapon_id=weapon_id,
            display_name=display_name,
            weapon_family=weapon_family,
            planner_provider="mock",
        )


class OpenAICompatibleLLMProvider:
    provider_id = "openai_compatible_llm"

    def __init__(self, config: OpenAICompatibleConfig) -> None:
        self.config = config

    def plan_weapon_spec(self, request: CreateWeaponRequest, *, weapon_id: str) -> Dict[str, Any]:
        if not self.config.api_key:
            raise LLMProviderError("PROVIDER_UNCONFIGURED", "OpenAI-compatible LLM API key is not configured.")
        if not self.config.model:
            raise LLMProviderError("PROVIDER_UNCONFIGURED", "OpenAI-compatible LLM model is not configured.")

        payload = {
            "model": self.config.model,
            "messages": [
                {
                    "role": "system",
                    "content": (
                        "You are Wushen Forge Art Director. Return only JSON for a fictional Unity game weapon art asset. "
                        "Do not include real-world manufacturable dimensions, blueprints, materials formulas, machining, or assembly instructions. "
                        "Input objects can be literal or abstract; you are encouraged to remap them into mythic weapon forms that remain one coherent subject. "
                        "The response must be directly consumable as WeaponDesignSpec fields and include clear visual details: "
                        "readable silhouette description, multi-zone materials, and prompt language that is game-grade 3D-friendly."
                    ),
                },
                {
                    "role": "user",
                    "content": (
                        f"Create one 3-to-2 Chinese fantasy divine weapon design spec for this prompt: {request.text}. "
                        "The weapon should look visually realistic as game art, but remain a fictional non-manufacturing asset. "
                        "If user text is a non-standard object, convert the object into a fantasy weapon archetype while keeping visual realism and game-readability. "
                        "Prioritize: strong main weapon shape, readable silhouette, layered ornamentation, and distinct material zones "
                        "for main body, accent structure, and glow core. Keep concept prompts single-subject and 3D-friendly."
                    ),
                },
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "weapon_design_spec",
                    "strict": False,
                    "schema": llm_output_schema(),
                },
            },
            "temperature": 0.7,
        }
        endpoint = self.config.base_url.rstrip("/") + "/chat/completions"
        http_request = urllib.request.Request(endpoint, data=json.dumps(payload).encode("utf-8"), method="POST")
        http_request.add_header("Content-Type", "application/json")
        http_request.add_header("Authorization", f"Bearer {self.config.api_key}")
        try:
            with urllib.request.urlopen(http_request, timeout=self.config.timeout_seconds) as response:
                data = json.loads(response.read().decode("utf-8"))
        except urllib.error.HTTPError as exc:
            if exc.code in {401, 403}:
                raise LLMProviderError("PROVIDER_AUTH_FAILED", "OpenAI-compatible provider rejected the API key.", False) from exc
            if exc.code == 429:
                raise LLMProviderError("RATE_LIMITED", "OpenAI-compatible provider rate limited the request.") from exc
            raise LLMProviderError("PROVIDER_TIMEOUT", f"OpenAI-compatible provider HTTP error {exc.code}.") from exc
        except Exception as exc:  # noqa: BLE001 - provider boundary converts external failures.
            raise LLMProviderError("PROVIDER_TIMEOUT", "OpenAI-compatible provider request failed.") from exc

        try:
            content = data["choices"][0]["message"]["content"]
            parsed = json.loads(content)
        except Exception as exc:  # noqa: BLE001 - provider JSON shape is external.
            raise LLMProviderError("INVALID_LLM_JSON", "OpenAI-compatible provider did not return valid JSON.") from exc

        fallback = build_fallback_weapon_spec(
            request,
            weapon_id=weapon_id,
            display_name=derive_display_name(request.text),
            weapon_family=derive_weapon_family(request.text),
            planner_provider="openai_compatible",
        )
        return normalize_llm_weapon_spec(parsed, fallback=fallback, weapon_id=weapon_id)


def llm_provider_from_env() -> LLMProvider:
    provider = os.environ.get("WUSHEN_LLM_PROVIDER", "mock").strip().lower()
    if provider == "openai_compatible":
        return OpenAICompatibleLLMProvider(
            OpenAICompatibleConfig(
                base_url=os.environ.get("WUSHEN_LLM_BASE_URL", os.environ.get("WUSHEN_OPENAI_BASE_URL", "https://api.openai.com/v1")),
                model=os.environ.get("WUSHEN_LLM_MODEL", os.environ.get("WUSHEN_OPENAI_MODEL", "")),
                api_key=read_secret_from_env("WUSHEN_LLM_API_KEY", "WUSHEN_LLM_API_KEY_FILE"),
                timeout_seconds=float(os.environ.get("WUSHEN_LLM_TIMEOUT_SECONDS", "60")),
            )
        )
    return MockLLMProvider()


def llm_provider_settings_from_env() -> list[ProviderSettings]:
    selected = os.environ.get("WUSHEN_LLM_PROVIDER", "mock").strip().lower()
    api_key = read_secret_from_env("WUSHEN_LLM_API_KEY", "WUSHEN_LLM_API_KEY_FILE")
    model = os.environ.get("WUSHEN_LLM_MODEL", os.environ.get("WUSHEN_OPENAI_MODEL", ""))
    base_url = os.environ.get("WUSHEN_LLM_BASE_URL", os.environ.get("WUSHEN_OPENAI_BASE_URL", "https://api.openai.com/v1"))
    planner = concept_planner_from_env()
    planner_config = getattr(planner, "config", None)
    return [
        ProviderSettings(
            provider_id="mock_llm",
            kind="llm",
            type="mock",
            display_name="Mock WeaponDesignSpec Planner",
            enabled=selected == "mock",
            status="configured" if selected == "mock" else "available",
            has_secret=False,
        ),
        ProviderSettings(
            provider_id="openai_compatible_llm",
            kind="llm",
            type="openai_compatible",
            display_name="OpenAI-compatible LLM",
            enabled=selected == "openai_compatible",
            status="configured" if api_key and model else "missing_config",
            base_url=base_url,
            has_secret=bool(api_key),
            updated_at=utc_now(),
        ),
        ProviderSettings(
            provider_id=planner.provider_id,
            kind="llm",
            type=planner.provider_type,
            display_name="ForgeCAD Concept Brief / Module Planner",
            enabled=True,
            status=(
                "configured"
                if planner.provider_type == "deterministic"
                or (
                    planner_config is not None
                    and planner_config.api_key
                    and planner_config.model
                )
                else "missing_config"
            ),
            base_url=(planner_config.base_url if planner_config is not None else None),
            has_secret=bool(planner_config and planner_config.api_key),
            updated_at=utc_now(),
        ),
    ]


def read_secret_from_env(value_name: str, file_name: str) -> Optional[str]:
    value = os.environ.get(value_name)
    if value:
        return value
    secret_file = os.environ.get(file_name)
    if secret_file:
        try:
            return open(secret_file, "r", encoding="utf-8").read().strip()
        except OSError:
            return None
    return None


QUALITY_NEGATIVE_PROMPT_EXCLUSIONS = [
    "real weapon blueprint",
    "manufacturing drawing",
    "dimensions",
    "material formula",
    "machining steps",
    "watermark",
    "unreadable text",
    "broken subject",
    "missing subject",
]


CREATIVE_RECAST_RULES = [
    {
        "id": "pants_cannon",
        "keywords": ["裤", "裤子", "pants", "pant", "trousers"],
        "weapon_family": "other",
        "display_name": "玄纱防弹裤",
        "prompt_focus": "recast the source object as a mythic waist-fired gauntlet cannon and keep it one coherent subject.",
        "visual_keywords": [
            "waist cannon architecture",
            "folded reinforced cloth armor",
            "mythic cartridge seams",
            "jade-gold safety glyphs",
            "single-subject weapon readout",
        ],
        "material_zones": [
            {"zone": "waist_chassis", "material": "forged lacquered mythic alloy", "notes": "hard shell around the belt spine"},
            {"zone": "chamber", "material": "tempered jade core", "notes": "glowing pressure reservoir"},
            {"zone": "flare", "material": "bronze cloth weave", "notes": "artificial flare skirt and stabilizing rib"},
        ],
        "silhouette": {
            "primary_shape": "waist-hugging segmented weapon body with front cannon chamber",
            "readability": "strong",
            "asymmetry": "high",
        },
        "prompt_suffix": "transform this idea into a divine waist weapon with reinforced seams, cartridge details, and a ritual firing core",
    },
    {
        "id": "staff_cannon",
        "keywords": ["棍", "棍子", "棒", "杆", "staff", "cane", "stick", "pole"],
        "weapon_family": "staff",
        "display_name": "神炮木棍",
        "prompt_focus": "show a long staff weapon that behaves like a compact cannon with a heavy forward fire mouth.",
        "visual_keywords": [
            "cannon-staff silhouette",
            "barrel bloom tip",
            "mythic recoil struts",
            "stacked talisman bands",
            "single-subject magical engineering",
        ],
        "material_zones": [
            {"zone": "shaft", "material": "aged blackwood core", "notes": "center spine with torque ridges"},
            {"zone": "muzzle", "material": "obsidian bronze", "notes": "front emitter and blast ring"},
            {"zone": "rings", "material": "gold filigree", "notes": "glyph bindings and recoil braces"},
        ],
        "silhouette": {
            "primary_shape": "long staff with expanded cannon-like muzzle and balanced rear grip",
            "readability": "strong",
            "asymmetry": "medium",
        },
        "prompt_suffix": "reinterpret the stick as an offensive staff-cannon with readable muzzle and stable hand position",
    },
    {
        "id": "chair_ram",
        "keywords": ["椅子", "chair", "凳", "凳子"],
        "weapon_family": "hammer",
        "display_name": "影座巨轭槌",
        "prompt_focus": "keep the silhouette as a compact impact hammer built from a throneshock chair form.",
        "visual_keywords": [
            "throne frame head mass",
            "compressed impact core",
            "ceremonial backplate",
            "single clean weapon subject",
            "impact aura",
        ],
        "material_zones": [
            {"zone": "seat", "material": "bronze-rimmed hardwood", "notes": "reinforced front impact block"},
            {"zone": "post", "material": "ironwood", "notes": "structural grip and spine"},
            {"zone": "ornament", "material": "jade inlay", "notes": "finishing runes"},
        ],
        "silhouette": {
            "primary_shape": "thick base-to-head mass with short balancing grip",
            "readability": "strong",
            "asymmetry": "low",
        },
        "prompt_suffix": "translate chair geometry into a playable mythic melee cannon-mace composition",
    },
    {
        "id": "branch_spike",
        "keywords": ["树枝", "树桩", "枝条", "branch", "branchy", "木枝"],
        "weapon_family": "spear",
        "display_name": "灵枢木枝矛",
        "prompt_focus": "make the organic branch shape into a sharp directional spear motif while keeping fantasy readability.",
        "visual_keywords": [
            "living wood spiral",
            "branch knot ornaments",
            "jade spike tip",
            "organic energy channels",
            "single-subject high-contrast silhouette",
        ],
        "material_zones": [
            {"zone": "trunk", "material": "spiral blackwood", "notes": "organic grip with carved runes"},
            {"zone": "tip", "material": "crystal-forged jade", "notes": "spear core and puncture edge"},
            {"zone": "tendril", "material": "bronze vine", "notes": "secondary decorative binders"},
        ],
        "silhouette": {
            "primary_shape": "organic spear with living knots and reinforced tip",
            "readability": "strong",
            "asymmetry": "medium",
        },
        "prompt_suffix": "turn the branch into a ceremonial forest weapon with readable spine and clear strike direction",
    },
]


def _normalize_text(value: str) -> str:
    return value.lower()


def extract_creative_recasts(text: str) -> list[dict[str, Any]]:
    if not text:
        return []
    raw_text = text.replace("\n", " ")
    lower_text = _normalize_text(raw_text)
    recasts: list[dict[str, Any]] = []
    for rule in CREATIVE_RECAST_RULES:
        for keyword in rule["keywords"]:
            if keyword in raw_text or keyword.lower() in lower_text:
                recasts.append(rule)
                break
    return recasts


def _resolve_creative_recast(text: str) -> Optional[dict[str, Any]]:
    recasts = extract_creative_recasts(text)
    return recasts[0] if recasts else None


def _merge_recast_profile(
    base_profile: dict[str, Any],
    recast_profile: Optional[dict[str, Any]],
) -> tuple[dict[str, Any], list[str]]:
    if recast_profile is None:
        return base_profile, []

    merged = dict(base_profile)
    if recast_profile.get("silhouette"):
        merged["silhouette"] = dict(recast_profile["silhouette"])
    if recast_profile.get("visual_keywords"):
        merged["visual_keywords"] = list(dict.fromkeys([*base_profile.get("visual_keywords", []), *recast_profile["visual_keywords"]]))
    if recast_profile.get("material_zones"):
        merged["material_zones"] = list(recast_profile["material_zones"]) + list(base_profile.get("material_zones", []))
    if recast_profile.get("prompt_focus"):
        merged["prompt_focus"] = recast_profile["prompt_focus"]
    suffix = recast_profile.get("prompt_suffix")
    suffix_items = [str(suffix)] if isinstance(suffix, str) and suffix else []
    return merged, suffix_items


FAMILY_DETAIL_PROFILES = {
    "sword": {
        "silhouette": {"primary_shape": "long single-edged blade with sculpted guard", "readability": "strong", "asymmetry": "medium"},
        "visual_keywords": ["long martial blade", "dragon crest", "layered engraving", "high-contrast silhouette", "jade inlay"],
        "material_zones": [
            {"zone": "blade", "material": "polished mythic alloy", "notes": "raised edge and reflective planes"},
            {"zone": "guard", "material": "engraved bronze", "notes": "lotus-and-rune trim"},
            {"zone": "core", "material": "jade emissive core", "notes": "soft glowing aura"},
        ],
        "prompt_focus": "blade spine has clear thrust-forward silhouette, pommel and guard create a readable silhouette frame",
    },
    "blade": {
        "silhouette": {"primary_shape": "wide crescent or arc blade with sweeping secondary arc", "readability": "strong", "asymmetry": "high"},
        "visual_keywords": ["crescent profile", "flame-groove edge", "ornate handle wrap", "shadow-heavy profile", "dynamic stance-friendly"],
        "material_zones": [
            {"zone": "blade", "material": "dark steel matte", "notes": "layered edge wear and bevel"},
            {"zone": "handle", "material": "brushed jade corewood", "notes": "high contrast grip pattern"},
            {"zone": "flare", "material": "gold ritual trim", "notes": "symbolic flame runes"},
        ],
        "prompt_focus": "emphasize broad cutting arc, forward balance and asymmetric flourish",
    },
    "spear": {
        "silhouette": {"primary_shape": "slim shaft with dramatic head spread", "readability": "strong", "asymmetry": "low"},
        "visual_keywords": ["long shaft", "sigil bands", "wind-trail carving", "thrust-ready form", "charged tip"],
        "material_zones": [
            {"zone": "shaft", "material": "antique bronze alloy", "notes": "grain-like ornamental carving"},
            {"zone": "head", "material": "obsidian-tooth edge", "notes": "etched cloud-rune channels"},
            {"zone": "binding", "material": "cloth-dyed leather", "notes": "tight hand-grip contrast"},
        ],
        "prompt_focus": "show a clear spearhead transition and elongated readout for long-distance recognition",
    },
    "halberd": {
        "silhouette": {"primary_shape": "double-ended polearm with cross guard wing", "readability": "strong", "asymmetry": "medium"},
        "visual_keywords": ["celestial halberd", "dual arc edge", "cloud blade", "battle standard symmetry", "energy pulse"],
        "material_zones": [
            {"zone": "head", "material": "storm-forged steel", "notes": "contrasting dual-edge silhouette"},
            {"zone": "pole", "material": "carved blackwood", "notes": "vertical rune bands"},
            {"zone": "rivet", "material": "brushed copper", "notes": "joint and spine highlights"},
        ],
        "prompt_focus": "use a visible centerline and lateral mass to keep halberd silhouette instantly readable",
    },
    "axe": {
        "silhouette": {"primary_shape": "high-impact haft with offset crescent blade", "readability": "strong", "asymmetry": "high"},
        "visual_keywords": ["off-axis head", "stone texture motif", "impact-ready stance", "broken crest ornament", "battle-crown edge"],
        "material_zones": [
            {"zone": "head", "material": "meteor iron", "notes": "chunky fracture-like surface"},
            {"zone": "haft", "material": "ironwood", "notes": "reinforced grip geometry"},
            {"zone": "socket", "material": "golded bronze", "notes": "stabilizer ring and rivets"},
        ],
        "prompt_focus": "show offset imbalance so the weapon feels heavy but controllable in game posture",
    },
    "bow": {
        "silhouette": {"primary_shape": "arched draw frame with central focal core", "readability": "strong", "asymmetry": "medium"},
        "visual_keywords": ["war bow", "celestial string arc", "phoenix feather pattern", "tactile bowstring", "ornamented limbs"],
        "material_zones": [
            {"zone": "limb", "material": "dark horn composite", "notes": "reinforced curvature and ridges"},
            {"zone": "string", "material": "jade-thread filament", "notes": "energy-infused visual string"},
            {"zone": "core", "material": "ember crystal", "notes": "pulsing power reservoir"},
        ],
        "prompt_focus": "keep the bow profile symmetrical in side view and legible from distance",
    },
    "crossbow": {
        "silhouette": {"primary_shape": "stacked cross-limbs around compact trigger block", "readability": "strong", "asymmetry": "low"},
        "visual_keywords": ["arcane mechanism", "gear stack", "spirit bolt channel", "tension ring", "ornamental braces"],
        "material_zones": [
            {"zone": "body", "material": "lacquered steel", "notes": "gear cavity and relief etching"},
            {"zone": "trigger", "material": "brass", "notes": "precise actuation cluster"},
            {"zone": "stock", "material": "smoked wood", "notes": "support ribbing and binding"},
        ],
        "prompt_focus": "balance mechanical complexity with clean outer edges for stable concept rendering",
    },
    "hammer": {
        "silhouette": {"primary_shape": "solid impact loop with bell-like head mass", "readability": "strong", "asymmetry": "medium"},
        "visual_keywords": ["impact mass", "temple-bell contour", "ornamental boss", "ringed aura", "weighted arc"],
        "material_zones": [
            {"zone": "head", "material": "dark brass", "notes": "hollowed ritual channels"},
            {"zone": "handle", "material": "woven scale", "notes": "grip loops and wraps"},
            {"zone": "rim", "material": "crimson enamel", "notes": "contrast edge highlight"},
        ],
        "prompt_focus": "exaggerate center mass while keeping strike direction clear",
    },
    "scythe": {
        "silhouette": {"primary_shape": "curved reaper blade attached to slender shaft", "readability": "strong", "asymmetry": "high"},
        "visual_keywords": ["sweeping crescent", "shadow hook", "mist ribbon", "moon arc", "dark jade inlay"],
        "material_zones": [
            {"zone": "blade", "material": "forged umbra steel", "notes": "crescent edge with matte depth"},
            {"zone": "shaft", "material": "bonewood", "notes": "spined reinforcement"},
            {"zone": "halo", "material": "pale luminescent crystal", "notes": "subtle glow for shape read"},
        ],
        "prompt_focus": "highlight the blade arc and make the weapon readable as a slicing form",
    },
    "staff": {
        "silhouette": {"primary_shape": "long central rod with clustered top ornaments", "readability": "strong", "asymmetry": "low"},
        "visual_keywords": ["arcane staff", "floating spheres", "stacked talisman bands", "rune halo", "vertical balance"],
        "material_zones": [
            {"zone": "shaft", "material": "polished blackwood", "notes": "grain-aligned grooves"},
            {"zone": "orb", "material": "moon jade", "notes": "soft emission node"},
            {"zone": "rings", "material": "silver alloy", "notes": "glyph-bearing ring set"},
        ],
        "prompt_focus": "keep top accents clustered and shaft vertical for clear magical-readability",
    },
    "umbrella": {
        "silhouette": {"primary_shape": "folded dome frame with elongated handle spine", "readability": "strong", "asymmetry": "medium"},
        "visual_keywords": ["mechanical parasol", "rippled cloth", "water rune lines", "compact shield form", "ornamental ribs"],
        "material_zones": [
            {"zone": "canopy", "material": "silken bronze-fiber cloth", "notes": "layered reflective folds"},
            {"zone": "ribs", "material": "bronze", "notes": "rib segmentation and filigree"},
            {"zone": "core", "material": "jade shard", "notes": "protective spirit core"},
        ],
        "prompt_focus": "preserve both closed and extended readability while remaining a single readable object",
    },
    "fan": {
        "silhouette": {"primary_shape": "multi-rib fan plates with central hand guard", "readability": "strong", "asymmetry": "medium"},
        "visual_keywords": ["battle fan", "wind trace", "feathered edge", "golden lattice", "energy slit"],
        "material_zones": [
            {"zone": "petals", "material": "bronze-plated steel", "notes": "ribbed metallic fan plates"},
            {"zone": "spine", "material": "obsidian handle", "notes": "grip pivot and support"},
            {"zone": "accent", "material": "spirit light", "notes": "faint outward energy trail"},
        ],
        "prompt_focus": "emphasize fan expansion direction so side silhouette is instantly readable",
    },
    "mechanical": {
        "silhouette": {"primary_shape": "gear-laden core with articulated frame", "readability": "strong", "asymmetry": "medium"},
        "visual_keywords": ["arcane machinery", "gear halo", "rotor veins", "symbolic interface", "power valve"],
        "material_zones": [
            {"zone": "frame", "material": "forged titanium", "notes": "articulated armor segments"},
            {"zone": "gear", "material": "copper-bronze alloy", "notes": "high-contrast rotary details"},
            {"zone": "visor", "material": "hologlaze", "notes": "procedural light panel"},
        ],
        "prompt_focus": "organize mechanical parts into distinct layers for clean 3D reconstruction",
    },
    "energy": {
        "silhouette": {"primary_shape": "ethereal emitter with symbolic core and reinforced shaft", "readability": "strong", "asymmetry": "medium"},
        "visual_keywords": ["energy conduit", "glyph channels", "holographic haze", "plasma edge", "mythic reactor"],
        "material_zones": [
            {"zone": "emitter", "material": "white-gold composite", "notes": "light-reflective emitter surface"},
            {"zone": "core", "material": "plasma jade", "notes": "strong local bloom"},
            {"zone": "housing", "material": "carbon weave", "notes": "structural lattice and cable routing"},
        ],
        "prompt_focus": "keep energy effects separated from hard geometry for clean shading and mesh boundaries",
    },
    "hybrid": {
        "silhouette": {"primary_shape": "transformable multi-mode geometry", "readability": "strong", "asymmetry": "high"},
        "visual_keywords": ["shape-shifting mode", "joint seam", "compact deployment", "mythic conversion", "dual-function geometry"],
        "material_zones": [
            {"zone": "base", "material": "obsidian carbon", "notes": "primary body shell"},
            {"zone": "joint", "material": "gold trim", "notes": "transform hinge and lock cues"},
            {"zone": "core", "material": "ember crystal", "notes": "mode change glow"},
        ],
        "prompt_focus": "show where each mode folds without breaking visual coherence",
    },
    "alien": {
        "silhouette": {"primary_shape": "irregular organic spire with bone-like ribs", "readability": "medium", "asymmetry": "high"},
        "visual_keywords": ["biomorphic edge", "bone shard", "xenolith crust", "dark aura", "otherworldly growth"],
        "material_zones": [
            {"zone": "spine", "material": "organic resin", "notes": "bone-like ridges"},
            {"zone": "plates", "material": "mineral crust", "notes": "pitted alien shell"},
            {"zone": "veins", "material": "electric jade", "notes": "faint glowing veins"},
        ],
        "prompt_focus": "maintain readable structural rhythm even with irregular growth patterns",
    },
    "mace": {
        "silhouette": {"primary_shape": "short handle with rounded impact bulb", "readability": "strong", "asymmetry": "medium"},
        "visual_keywords": ["blunt impact", "crystal boss", "ceremonial chain loops", "impact chamber", "defensive grip"],
        "material_zones": [
            {"zone": "head", "material": "obsidian bronze", "notes": "rounded boss with engraved ridges"},
            {"zone": "handle", "material": "knotted ironwood", "notes": "grip texture for game handling"},
            {"zone": "ring", "material": "gold filigree", "notes": "decorative impact halo"},
        ],
        "prompt_focus": "highlight the heavy striking silhouette while preserving compactness",
    },
    "trident": {
        "silhouette": {"primary_shape": "triple-prong front cluster on short-to-mid shaft", "readability": "strong", "asymmetry": "high"},
        "visual_keywords": ["three-prong focus", "tri-ridge crown", "salt-wave carving", "mythic throw", "channel marks"],
        "material_zones": [
            {"zone": "shaft", "material": "weathered alloy", "notes": "central structural spine"},
            {"zone": "fork", "material": "jade alloy", "notes": "triple prong silhouette"},
            {"zone": "bindings", "material": "red silk cord", "notes": "visual reinforcement wraps"},
        ],
        "prompt_focus": "make the three-prong shape read clearly from front and side",
    },
    "dagger": {
        "silhouette": {"primary_shape": "compact double-edge core with narrow pommel", "readability": "strong", "asymmetry": "low"},
        "visual_keywords": ["compact lethal shape", "knife guard", "narrow waist", "jade sigil", "close-combat speed"],
        "material_zones": [
            {"zone": "blade", "material": "black cobalt", "notes": "micro-etched edge facets"},
            {"zone": "hilt", "material": "bronze weave", "notes": "tight grip geometry"},
            {"zone": "pommel", "material": "moonstone", "notes": "focal contrast point"},
        ],
        "prompt_focus": "emphasize hand-held scale and short-form balance",
    },
}

DEFAULT_FAMILY_PROFILE = {
    "silhouette": {"primary_shape": "strong fantasy weapon motif", "readability": "strong", "asymmetry": "medium"},
    "visual_keywords": ["Chinese fantasy", "cel shaded outline", "ornamental metal", "jade energy core", "high contrast"],
    "material_zones": [
        {"zone": "main_body", "material": "stylized dark metal", "notes": "decorative game asset surface"},
        {"zone": "ornament", "material": "engraved gold trim", "notes": "fantasy motif only"},
        {"zone": "core", "material": "emissive jade energy", "notes": "fictional glow element"},
    ],
    "prompt_focus": "keep the weapon form clear and centered with readable silhouette",
}


def _family_profile(weapon_family: str) -> dict[str, Any]:
    return FAMILY_DETAIL_PROFILES.get(weapon_family, DEFAULT_FAMILY_PROFILE)


def _build_fallback_prompt(
    request_text: str,
    family_profile: dict[str, Any],
    creative_suffixes: Optional[list[str]] = None,
) -> str:
    prompt_focus = family_profile.get("prompt_focus", "")
    silhouette = family_profile.get("silhouette", {})
    visual_keywords = ", ".join(family_profile.get("visual_keywords", []))
    material_layers = ", ".join(
        f"{zone.get('zone', 'zone')}:{zone.get('material', '')}".rstrip(":") for zone in family_profile.get("material_zones", [])
    )
    if not material_layers:
        material_layers = "stylized dark metal, engraved gold trim, emissive jade energy"
    creative_suffix = " ".join(term.strip() for term in creative_suffixes or [] if str(term).strip())
    return (
        f"{request_text}, 3渲2国风神兵, fictional high-realism game art, Unity-ready weapon concept. "
        f"Single clean subject, full weapon readout, neutral background, no collage or extra props, "
        f"strong silhouette and stable lighting. {prompt_focus} "
        f"{creative_suffix + ' ' if creative_suffix else ''}"
        f"Silhouette notes: {silhouette.get('primary_shape', '')}; readability:{silhouette.get('readability', 'strong')}; "
        f"asymmetry:{silhouette.get('asymmetry', 'medium')}. "
        f"Material layers: {material_layers}. "
        f"Style tags: {visual_keywords}. "
    )


def ensure_negative_prompt_exclusions(value: str) -> str:
    prompt = value.strip()
    lower_prompt = prompt.lower()
    missing = [term for term in QUALITY_NEGATIVE_PROMPT_EXCLUSIONS if term not in lower_prompt]
    if not prompt:
        return ", ".join(missing)
    if missing:
        prompt = f"{prompt}, {', '.join(missing)}"
    return prompt


def normalize_llm_weapon_spec(parsed: Dict[str, Any], *, fallback: Dict[str, Any], weapon_id: str) -> Dict[str, Any]:
    spec = dict(fallback)
    for key in [
        "name",
        "weapon_family",
        "fantasy_category",
        "silhouette",
        "visual_keywords",
        "color_palette",
        "material_zones",
        "toon_rules",
        "generation",
    ]:
        if key in parsed:
            spec[key] = parsed[key]
    spec["schema_version"] = "WeaponDesignSpec@1"
    spec["id"] = weapon_id
    spec["style"] = "3渲2国风神兵"
    spec["weapon_family"] = spec.get("weapon_family") if spec.get("weapon_family") in WEAPON_FAMILIES else fallback["weapon_family"]
    spec["fantasy_category"] = spec.get("fantasy_category") if spec.get("fantasy_category") in FANTASY_CATEGORIES else "custom"
    spec["safety_boundary"] = {"real_world_manufacturing_details": False}
    generation = dict(fallback["generation"])
    generation.update(spec.get("generation") or {})
    generation["negative_prompt"] = generation.get("negative_prompt") or fallback["generation"]["negative_prompt"]
    generation["negative_prompt"] = ensure_negative_prompt_exclusions(generation["negative_prompt"])
    spec["generation"] = generation
    spec["unity_target"] = fallback["unity_target"]
    spec["created_at"] = fallback["created_at"]
    return spec


def build_fallback_weapon_spec(
    request: CreateWeaponRequest,
    *,
    weapon_id: str,
    display_name: str,
    weapon_family: str,
    planner_provider: str,
) -> Dict[str, Any]:
    family_profile = _family_profile(weapon_family)
    creative_profile = _resolve_creative_recast(request.text)
    merged_profile, creative_suffixes = _merge_recast_profile(family_profile, creative_profile)
    prompt = _build_fallback_prompt(request.text, merged_profile, creative_suffixes)
    return {
        "schema_version": "WeaponDesignSpec@1",
        "id": weapon_id,
        "name": display_name,
        "style": "3渲2国风神兵",
        "weapon_family": weapon_family,
        "fantasy_category": "custom",
        "silhouette": merged_profile["silhouette"],
        "visual_keywords": merged_profile["visual_keywords"],
        "color_palette": {"primary": "#161616", "secondary": "#C79A3A", "accent": "#D8422A", "glow": "#FF7A32"},
        "material_zones": merged_profile["material_zones"],
        "toon_rules": {"outline": "strong", "shadow_steps": 2, "rim_light": "warm", "emission": "localized"},
        "generation": {
            "concept_prompt": prompt,
            "negative_prompt": ensure_negative_prompt_exclusions(""),
            "seed": request.generation_options.seed,
            "provider": planner_provider,
        },
        "unity_target": {
            "format": request.target.output_format,
            "scale_policy": "normalized_game_asset_scale",
            "scale_contract": {
                "longest_axis_normalized_to": 1.0,
                "unit_semantics": "game_relative",
                "forbid_real_world_dimensions": True,
            },
            "orientation_policy": {
                "coordinate_system": "gltf_y_up_unity_compatible",
                "long_axis": "+Y",
                "forward_axis": "+Z",
                "up_axis": "+Y",
                "pivot": "grip_center",
                "fallback_pivot": "bounding_box_center",
                "apply_transforms_before_export": True,
            },
            "model_3d": {
                "target_format": "glb",
                "provider_strategy": "mock_deferred",
                "source_image_role": "concept_image",
                "model_sheet_allowed": True,
                "expected_outputs": [
                    "model_generation_input",
                    "rough_raw_glb",
                    "rough_normalized_glb",
                    "rough_optimized_glb",
                    "rough_preview_png",
                    "unity_material_json",
                    "quality_report",
                ],
            },
            "material_profile": "wushen_toon_weapon_v1",
        },
        "safety_boundary": {"real_world_manufacturing_details": False},
        "created_at": utc_now(),
    }


def derive_display_name(text: str) -> str:
    normalized = text.strip().replace("\n", " ")
    if not normalized:
        return "未命名国风神兵"
    creative_profile = _resolve_creative_recast(normalized)
    if creative_profile and creative_profile.get("display_name"):
        return creative_profile["display_name"]
    lower_normalized = normalized.lower()
    if "三叉" in normalized or "三叉戟" in normalized or "trident" in lower_normalized:
        return "玄潮三叉戟"
    if "狼牙棒" in normalized or "钉头" in normalized or "铁槌" in normalized or "锤" in normalized:
        return "玄锤月裂"
    if "匕" in normalized or "匕首" in normalized or "dagger" in lower_normalized:
        return "疾月匕锋"
    if "剑" in normalized:
        return "赤霄龙纹斩"
    if "弓" in normalized:
        return "天弦破云弓"
    if "枪" in normalized:
        return "玄雷破阵枪"
    return "未命名国风神兵"


def derive_weapon_family(text: str) -> str:
    if not text:
        return "other"
    creative_profile = _resolve_creative_recast(text)
    if creative_profile and creative_profile.get("weapon_family"):
        return creative_profile["weapon_family"]

    normalized = text.lower()
    mapping = [
        ("mace", "mace"),
        ("trident", "trident"),
        ("dagger", "dagger"),
        ("crossbow", "crossbow"),
        ("cross-bow", "crossbow"),
        ("spear", "spear"),
        ("scythe", "scythe"),
        ("halberd", "halberd"),
        ("mechanical", "mechanical"),
        ("机械", "mechanical"),
        ("alien", "alien"),
        ("hybrid", "hybrid"),
        ("staff", "staff"),
        ("bow", "bow"),
        ("sword", "sword"),
        ("blade", "blade"),
        ("axe", "axe"),
        ("hammer", "hammer"),
        ("fan", "fan"),
        ("umbrella", "umbrella"),
        ("energy", "energy"),
        ("狼牙棒", "mace"),
        ("铁槌", "mace"),
        ("重锤", "mace"),
        ("三叉戟", "trident"),
        ("三叉", "trident"),
        ("匕首", "dagger"),
        ("短刀", "dagger"),
        ("匕", "dagger"),
        ("弩", "crossbow"),
        ("戟", "halberd"),
        ("镰", "scythe"),
        ("斧", "axe"),
        ("锤", "hammer"),
        ("扇", "fan"),
        ("伞", "umbrella"),
        ("杖", "staff"),
        ("棍", "staff"),
        ("刀", "blade"),
        ("弓", "bow"),
        ("矛", "spear"),
        ("枪", "spear"),
        ("剑", "sword"),
    ]
    for keyword, family in mapping:
        if keyword in text or keyword in normalized:
            return family
    return "other"


def llm_output_schema() -> Dict[str, Any]:
    return {
        "type": "object",
        "additionalProperties": True,
        "properties": {
            "name": {"type": "string"},
            "weapon_family": {"type": "string", "enum": sorted(WEAPON_FAMILIES)},
            "fantasy_category": {"type": "string", "enum": sorted(FANTASY_CATEGORIES)},
            "silhouette": {"type": "object"},
            "visual_keywords": {"type": "array", "items": {"type": "string"}},
            "color_palette": {"type": "object"},
            "material_zones": {"type": "array", "items": {"type": "object"}},
            "toon_rules": {"type": "object"},
            "generation": {"type": "object"},
        },
        "required": ["name", "weapon_family", "visual_keywords", "material_zones", "generation"],
    }


WEAPON_FAMILIES = {
    "sword",
    "blade",
    "spear",
    "halberd",
    "axe",
    "bow",
    "crossbow",
    "hammer",
    "scythe",
    "staff",
    "umbrella",
    "fan",
    "mechanical",
    "energy",
    "hybrid",
    "alien",
    "mace",
    "trident",
    "dagger",
    "other",
}

FANTASY_CATEGORIES = {
    "dragon_relic",
    "phoenix_relic",
    "celestial",
    "demonic",
    "talisman",
    "elemental",
    "jade_spirit",
    "bone_relic",
    "crystal_core",
    "mechanical_arcane",
    "custom",
}
