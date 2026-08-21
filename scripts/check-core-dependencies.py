#!/usr/bin/env python3
"""Fail-closed OC-01 audit of the package-scoped core dependency baseline."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib
from typing import Any, cast

CORE_NAME = "contextmesh"
SALIENCE_NAME = "contextmesh-salience"
EXPECTED_WORKSPACE_MEMBERS = 2
EXPECTED_CORE_REACHABLE = 320
EXPECTED_CORE_EXTERNAL = 319
EXPECTED_LOCK_PACKAGES = 321
BASELINE_EXTERNAL_SHA256 = "ae86da65ff5138bb51836d303ec9370ad9da8c8f112ad84ad59b5e362136113d"
BASELINE_CORE_DIRECT_SHA256 = "47b7df1d09c960c0ca1734a73fbb7974407d9a7aa240b04307b9aaa3399a76ef"
BASELINE_SALIENCE_DIRECT_SHA256 = "37ea4732021965e1eace50fc39b523bb28b3fb2dd0484ce4fadf246a2e766914"
BASELINE_FEATURE_TREE_SHA256 = "658b4fe016b1bc8ba748d31d88f61216df06afbd2c931059579bdff8375f461c"
FORBIDDEN_DEPENDENCY_TOKENS = (
    "candle", "embedding", "fastembed", "judge", "llama", "model", "onnx",
    "openai", "ort", "qdrant", "rerank", "reqwest", "rusqlite", "sqlx",
    "surrealdb", "tantivy", "ureq", "vector",
)
COMMAND_TIMEOUT_SECONDS = 120


class AuditError(Exception):
    """Internal fail-closed audit marker; details are intentionally not emitted."""


def require(condition: bool) -> None:
    if not condition:
        raise AuditError


def canonical_sha256(value: Any) -> str:
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def indexed_packages(metadata: dict[str, Any]) -> dict[str, dict[str, Any]]:
    packages = metadata.get("packages")
    require(isinstance(packages, list))
    packages = cast(list[Any], packages)
    result: dict[str, dict[str, Any]] = {}
    for package in packages:
        require(isinstance(package, dict) and isinstance(package.get("id"), str))
        require(package["id"] not in result)
        result[package["id"]] = package
    return result


def named_package(metadata: dict[str, Any], name: str) -> dict[str, Any]:
    matches = [package for package in indexed_packages(metadata).values() if package.get("name") == name]
    require(len(matches) == 1)
    return matches[0]


def reachable_ids(metadata: dict[str, Any], start: str) -> set[str]:
    packages = indexed_packages(metadata)
    resolve = metadata.get("resolve")
    require(isinstance(resolve, dict) and isinstance(resolve.get("nodes"), list))
    resolve = cast(dict[str, Any], resolve)
    resolve_nodes = cast(list[Any], resolve["nodes"])
    nodes: dict[str, dict[str, Any]] = {}
    for node in resolve_nodes:
        require(isinstance(node, dict) and isinstance(node.get("id"), str))
        require(node["id"] not in nodes and isinstance(node.get("deps"), list))
        nodes[node["id"]] = node
    require(start in packages and start in nodes)
    found: set[str] = set()
    pending = [start]
    while pending:
        package_id = pending.pop()
        if package_id in found:
            continue
        require(package_id in packages and package_id in nodes)
        found.add(package_id)
        dependencies = cast(list[Any], nodes[package_id]["deps"])
        for dependency in dependencies:
            require(isinstance(dependency, dict) and isinstance(dependency.get("pkg"), str))
            require(dependency["pkg"] in packages and dependency["pkg"] in nodes)
            pending.append(dependency["pkg"])
    return found


def dependency_signature(package: dict[str, Any], root: Path) -> list[dict[str, Any]]:
    dependencies = package.get("dependencies")
    require(isinstance(dependencies, list))
    dependencies = cast(list[Any], dependencies)
    fields = (
        "name", "source", "req", "kind", "rename", "optional",
        "uses_default_features", "features", "target", "registry", "path",
    )
    normalized = []
    for dependency in dependencies:
        require(isinstance(dependency, dict))
        record = {field: dependency.get(field) for field in fields}
        require(isinstance(record["features"], list))
        record["features"] = sorted(record["features"])
        if record["path"] is not None:
            require(isinstance(record["path"], str))
            dependency_path = Path(record["path"]).resolve()
            record["path"] = "<WORKSPACE>" if dependency_path == root else str(dependency_path)
        normalized.append(record)
    return sorted(
        normalized,
        key=lambda item: (
            item["name"], str(item["kind"]), str(item["target"]), str(item["rename"]),
        ),
    )


def external_identities(packages: list[dict[str, Any]]) -> list[tuple[str, str, str]]:
    identities = []
    for package in packages:
        source = package.get("source")
        if source is not None:
            require(isinstance(package.get("name"), str))
            require(isinstance(package.get("version"), str))
            require(isinstance(source, str) and (source.startswith("registry+") or source.startswith("git+")))
            identities.append((package["name"], package["version"], source))
    require(len(identities) == len(set(identities)))
    return sorted(identities)


def cargo_executable() -> str:
    for variable in ("OC01_CARGO", "CARGO"):
        candidate = os.environ.get(variable)
        if candidate:
            return candidate
    discovered = shutil.which("cargo")
    if discovered is not None:
        return discovered
    cargo_home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
    fallback = cargo_home / "bin" / "cargo"
    require(fallback.is_file())
    return str(fallback)


def cargo_command(root: Path, *args: str) -> bytes:
    environment = os.environ.copy()
    environment["CARGO_NET_OFFLINE"] = "true"
    completed = subprocess.run(
        [cargo_executable(), *args], cwd=root, env=environment, check=False,
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        timeout=COMMAND_TIMEOUT_SECONDS,
    )
    require(completed.returncode == 0)
    return completed.stdout


def full_audit(root: Path) -> dict[str, Any]:
    metadata = json.loads(cargo_command(root, "metadata", "--locked", "--format-version", "1"))
    require(isinstance(metadata, dict))
    packages_by_id = indexed_packages(metadata)
    packages = list(packages_by_id.values())
    core = named_package(metadata, CORE_NAME)
    salience = named_package(metadata, SALIENCE_NAME)

    workspace_members = metadata.get("workspace_members")
    require(isinstance(workspace_members, list) and len(workspace_members) == EXPECTED_WORKSPACE_MEMBERS)
    workspace_members = cast(list[str], workspace_members)
    require(len(set(workspace_members)) == len(workspace_members))
    require(set(workspace_members) == {core["id"], salience["id"]})
    workspace_local = [packages_by_id[item] for item in workspace_members]
    require(all(package.get("source") is None for package in workspace_local))

    core_reachable = reachable_ids(metadata, core["id"])
    salience_reachable = reachable_ids(metadata, salience["id"])
    require(salience["id"] not in core_reachable)
    require(core["id"] in salience_reachable)
    core_external = external_identities([packages_by_id[item] for item in core_reachable])
    require(len(core_reachable) == EXPECTED_CORE_REACHABLE)
    require(len(core_external) == EXPECTED_CORE_EXTERNAL)
    require(canonical_sha256(core_external) == BASELINE_EXTERNAL_SHA256)
    require(canonical_sha256(dependency_signature(core, root)) == BASELINE_CORE_DIRECT_SHA256)

    require(salience.get("version") == "0.1.0")
    require(salience.get("edition") == "2024")
    require(salience.get("rust_version") == "1.97")
    require(salience.get("source") is None)
    salience_signature = dependency_signature(salience, root)
    require(canonical_sha256(salience_signature) == BASELINE_SALIENCE_DIRECT_SHA256)
    forbidden = sorted({
        dependency["name"] for dependency in salience_signature
        if any(token in dependency["name"].lower() for token in FORBIDDEN_DEPENDENCY_TOKENS)
    })
    require(not forbidden)

    all_external = external_identities(packages)
    require(canonical_sha256(all_external) == BASELINE_EXTERNAL_SHA256)
    lock = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))
    lock_packages = lock.get("package")
    require(isinstance(lock_packages, list) and len(lock_packages) == EXPECTED_LOCK_PACKAGES)
    lock_packages = cast(list[dict[str, Any]], lock_packages)
    lock_external = external_identities(lock_packages)
    require(canonical_sha256(lock_external) == BASELINE_EXTERNAL_SHA256)

    baseline_tree = (root / "cargo-tree-oa05-features.txt").read_bytes()
    require(hashlib.sha256(baseline_tree).hexdigest() == BASELINE_FEATURE_TREE_SHA256)
    current_tree = cargo_command(root, "tree", "-p", CORE_NAME, "--locked", "-e", "features")
    lines = current_tree.splitlines(keepends=True)
    require(bool(lines) and lines[0].startswith(b"contextmesh v0.1.0 (") and lines[0].rstrip().endswith(b")"))
    lines[0] = b"contextmesh v0.1.0 (<WORKSPACE>)\n"
    current_tree = b"".join(lines)
    require(current_tree == baseline_tree)

    return {
        "core_direct_dependencies_unchanged": True,
        "core_reachable_external_packages": len(core_external),
        "core_reachable_packages": len(core_reachable),
        "core_registry_closure_sha256": canonical_sha256(core_external),
        "core_registry_closure_unchanged": True,
        "feature_tree_sha256": hashlib.sha256(current_tree).hexdigest(),
        "forbidden_capabilities": forbidden,
        "lock_packages": len(lock_packages),
        "new_registry_identities": 0,
        "salience_direct_dependencies_exact": True,
        "workspace_local_packages": len(workspace_local),
        "workspace_members": len(workspace_members),
    }


def named_reachability(path: Path) -> dict[str, int]:
    metadata = json.loads(path.read_text(encoding="utf-8"))
    require(isinstance(metadata, dict))
    core = named_package(metadata, CORE_NAME)
    return {"core_reachable": len(reachable_ids(metadata, core["id"]))}


def main() -> int:
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--named-reachability", type=Path)
    arguments = parser.parse_args()
    try:
        if arguments.named_reachability is not None:
            report = named_reachability(arguments.named_reachability)
        else:
            report = full_audit(Path(__file__).resolve().parent.parent)
        print(json.dumps(report, sort_keys=True, separators=(",", ":")))
        return 0
    except (
        AuditError, KeyError, OSError, ValueError, json.JSONDecodeError,
        subprocess.SubprocessError, tomllib.TOMLDecodeError,
    ):
        print("dependency audit failed", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
