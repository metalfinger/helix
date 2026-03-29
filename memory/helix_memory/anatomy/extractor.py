"""Per-file extraction: language, description, symbols, token estimate."""

from __future__ import annotations

import ast
import re
from pathlib import Path

# -- Language map ----------------------------------------------------------

LANG_MAP = {
    ".py": "python", ".pyw": "python", ".pyi": "python",
    ".rs": "rust",
    ".ts": "typescript", ".tsx": "typescript", ".mts": "typescript",
    ".js": "javascript", ".jsx": "javascript", ".mjs": "javascript",
    ".cpp": "cpp", ".cc": "cpp", ".cxx": "cpp", ".h": "cpp", ".hpp": "cpp",
    ".c": "c",
    ".go": "go",
    ".sh": "shell", ".bash": "shell", ".zsh": "shell",
    ".ps1": "powershell", ".psm1": "powershell",
    ".yaml": "yaml", ".yml": "yaml",
    ".toml": "toml",
    ".json": "json",
    ".md": "markdown", ".mdx": "markdown",
    ".html": "html", ".htm": "html",
    ".css": "css", ".scss": "css", ".less": "css",
    ".sql": "sql",
    ".lua": "lua",
    ".zig": "zig",
    ".dart": "dart",
    ".swift": "swift",
    ".kt": "kotlin", ".kts": "kotlin",
    ".java": "java",
    ".rb": "ruby",
    ".ex": "elixir", ".exs": "elixir",
    ".php": "php",
    ".vue": "vue",
    ".svelte": "svelte",
    ".astro": "astro",
    ".proto": "protobuf",
    ".graphql": "graphql", ".gql": "graphql",
    ".tf": "terraform",
    ".dockerfile": "docker",
}

PROSE_LANGUAGES = frozenset({"markdown", "yaml", "toml", "json", "html", "graphql", "protobuf"})

# Max bytes to read per file for extraction
MAX_READ = 8192


def detect_language(path: Path) -> str:
    ext = path.suffix.lower()
    if path.name.lower() == "dockerfile":
        return "docker"
    if path.name.lower() == "makefile":
        return "make"
    return LANG_MAP.get(ext, "unknown")


def estimate_tokens(size_bytes: int, language: str) -> int:
    divisor = 4.0 if language in PROSE_LANGUAGES else 3.5
    return max(1, int(size_bytes / divisor))


# -- Symbol extraction per language ----------------------------------------

def _extract_python(text: str, path: Path) -> tuple[str, list[str]]:
    """Use ast for Python, fall back to regex."""
    symbols = []
    description = ""
    try:
        tree = ast.parse(text)
        # Module docstring
        if (tree.body and isinstance(tree.body[0], ast.Expr)
                and isinstance(tree.body[0].value, (ast.Constant, ast.Str))):
            val = tree.body[0].value
            doc = val.value if isinstance(val, ast.Constant) else val.s
            description = doc.strip().split("\n")[0][:120]
        for node in tree.body:
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                symbols.append(node.name)
            elif isinstance(node, ast.ClassDef):
                symbols.append(node.name)
    except SyntaxError:
        # Fallback to regex
        symbols = re.findall(r"^(?:class|def|async\s+def)\s+(\w+)", text, re.MULTILINE)
    return description, symbols


def _extract_rust(text: str) -> tuple[str, list[str]]:
    symbols = re.findall(r"(?:pub\s+)?(?:fn|struct|enum|trait|type|const|static)\s+(\w+)", text)
    # First //! doc comment
    m = re.search(r"^//!\s*(.+)", text, re.MULTILINE)
    desc = m.group(1).strip()[:120] if m else ""
    return desc, symbols


def _extract_typescript(text: str) -> tuple[str, list[str]]:
    symbols = re.findall(
        r"export\s+(?:default\s+)?(?:function|class|const|let|var|type|interface|enum)\s+(\w+)",
        text,
    )
    if not symbols:
        symbols = re.findall(r"^(?:function|class|const|let|var)\s+(\w+)", text, re.MULTILINE)
    # First JSDoc or // comment
    m = re.search(r"^/\*\*\s*\n?\s*\*?\s*(.+?)(?:\n|\*/)", text)
    if not m:
        m = re.search(r"^//\s*(.+)", text, re.MULTILINE)
    desc = m.group(1).strip()[:120] if m else ""
    return desc, symbols


def _extract_cpp(text: str) -> tuple[str, list[str]]:
    symbols = []
    # UE5 macros
    for macro in ("UCLASS", "USTRUCT", "UENUM"):
        for m in re.finditer(rf"{macro}\([^)]*\)\s*\n?\s*(?:class|struct|enum)\s+\w*\s*(\w+)", text):
            symbols.append(m.group(1))
    # Regular C++ declarations
    symbols += re.findall(r"^(?:class|struct|enum)\s+(\w+)", text, re.MULTILINE)
    symbols += re.findall(r"^\w[\w:<>*& ]*\s+(\w+)\s*\(", text, re.MULTILINE)
    # First comment
    m = re.search(r"^/\*\*?\s*\n?\s*\*?\s*(.+?)(?:\n|\*/)", text)
    if not m:
        m = re.search(r"^//\s*(.+)", text, re.MULTILINE)
    desc = m.group(1).strip()[:120] if m else ""
    return desc, list(dict.fromkeys(symbols))  # dedup preserving order


def _extract_go(text: str) -> tuple[str, list[str]]:
    symbols = re.findall(r"^(?:func|type)\s+(\w+)", text, re.MULTILINE)
    m = re.search(r"^//\s*(.+)", text, re.MULTILINE)
    desc = m.group(1).strip()[:120] if m else ""
    return desc, symbols


def _extract_shell(text: str) -> tuple[str, list[str]]:
    symbols = re.findall(r"^(\w+)\s*\(\)", text, re.MULTILINE)
    m = re.search(r"^#\s*(.+)", text, re.MULTILINE)
    desc = m.group(1).strip()[:120] if m else ""
    # Skip shebangs
    if desc.startswith("!"):
        lines = [l for l in text.splitlines() if l.startswith("#") and not l.startswith("#!")]
        desc = lines[0][1:].strip()[:120] if lines else ""
    return desc, symbols


def _extract_generic(text: str) -> tuple[str, list[str]]:
    """Fallback: grab first comment line as description."""
    for line in text.splitlines()[:20]:
        stripped = line.strip()
        if stripped.startswith("#") and not stripped.startswith("#!"):
            return stripped.lstrip("# ").strip()[:120], []
        if stripped.startswith("//"):
            return stripped.lstrip("/ ").strip()[:120], []
        if stripped.startswith("/*"):
            content = stripped.lstrip("/* ").rstrip("*/").strip()
            if content:
                return content[:120], []
    return "", []


# -- Filename-based fallback descriptions ----------------------------------

def _infer_from_name(path: Path) -> str:
    name = path.name.lower()
    stem = path.stem.lower()

    if name == "__init__.py":
        return "Package init"
    if name in ("main.py", "main.rs", "main.go", "main.ts", "main.tsx", "index.ts", "index.js"):
        return "Entry point"
    if name in ("dockerfile", "docker-compose.yml", "docker-compose.yaml"):
        return "Docker configuration"
    if name == "makefile":
        return "Build configuration"
    if name in ("readme.md", "readme.txt", "readme"):
        return "Project documentation"
    if name in (".env.example", ".env.template"):
        return "Environment variable template"
    if name in ("pyproject.toml", "setup.py", "setup.cfg"):
        return "Python package configuration"
    if name == "cargo.toml":
        return "Rust package configuration"
    if name in ("package.json", "tsconfig.json"):
        return "Node.js/TypeScript configuration"
    if stem.startswith("test_") or stem.endswith("_test"):
        return f"Tests for {stem.replace('test_', '').replace('_test', '')}"
    if stem.startswith("test") and not stem == "test":
        return f"Tests for {stem[4:]}"
    if stem.endswith(".spec"):
        return f"Tests for {stem.replace('.spec', '')}"
    if stem.endswith(".test"):
        return f"Tests for {stem.replace('.test', '')}"
    return ""


# -- Main extraction function ----------------------------------------------

def extract_file_info(path: Path) -> dict:
    """Extract language, description, symbols, and token estimate for a file."""
    language = detect_language(path)

    try:
        size = path.stat().st_size
        mtime = path.stat().st_mtime
    except OSError:
        return {
            "language": language,
            "description": "Unreadable file",
            "tokens": 0,
            "size_bytes": 0,
            "symbols": [],
            "last_modified": 0,
        }

    tokens = estimate_tokens(size, language)

    if size == 0:
        return {
            "language": language,
            "description": "Empty file",
            "tokens": 0,
            "size_bytes": 0,
            "symbols": [],
            "last_modified": mtime,
        }

    try:
        text = path.read_text(encoding="utf-8", errors="replace")[:MAX_READ]
    except Exception:
        return {
            "language": language,
            "description": "Unreadable file",
            "tokens": tokens,
            "size_bytes": size,
            "symbols": [],
            "last_modified": mtime,
        }

    # Extract based on language
    desc, symbols = "", []
    if language == "python":
        desc, symbols = _extract_python(text, path)
    elif language == "rust":
        desc, symbols = _extract_rust(text)
    elif language in ("typescript", "javascript", "vue", "svelte", "astro"):
        desc, symbols = _extract_typescript(text)
    elif language in ("cpp", "c"):
        desc, symbols = _extract_cpp(text)
    elif language == "go":
        desc, symbols = _extract_go(text)
    elif language in ("shell", "powershell"):
        desc, symbols = _extract_shell(text)
    else:
        desc, symbols = _extract_generic(text)

    # Fallback to filename inference
    if not desc:
        desc = _infer_from_name(path)
    if not desc:
        desc = f"{language} source file"

    # Cap symbols at 20
    symbols = symbols[:20]

    return {
        "language": language,
        "description": desc,
        "tokens": tokens,
        "size_bytes": size,
        "symbols": symbols,
        "last_modified": mtime,
    }
