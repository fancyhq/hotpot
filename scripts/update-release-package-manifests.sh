#!/usr/bin/env bash
# Script to update Chocolatey package files with the release version and SHA256.
# 更新 Chocolatey 包文件中的 release 版本和 SHA256 校验和。
#
# Usage:
#   scripts/update-release-package-manifests.sh <tag> <sha256_dir>
#
# Arguments:
#   tag         - Release tag, e.g. "hotpot-v0.3.2"
#   sha256_dir  - Directory containing the Windows SHA256 checksum file, e.g.
#                 "hotpot-hotpot-v0.3.2-windows-x86_64.zip.sha256"
#
# Example:
#   scripts/update-release-package-manifests.sh hotpot-v0.3.2 ./sha256

set -euo pipefail

# Extract SHA256 hash from a .sha256 file.
# 从 .sha256 文件提取 SHA256 哈希值。
get_hash() {
  local file="$1"
  if [[ ! -f "$file" ]]; then
    echo "Error: SHA256 file not found: $file" >&2
    exit 1
  fi
  awk '{print $1}' "$file"
}

# Perform portable sed in-place editing across macOS and Linux.
# 在 macOS 和 Linux 上执行可移植的 sed 原地编辑。
sed_i() {
  local expr="$1"
  local file="$2"
  if [[ "$(uname -s)" == "Darwin" ]]; then
    sed -i '' "$expr" "$file"
  else
    sed -i "$expr" "$file"
  fi
}

# Compute semver from a release tag.
# 从 release tag 计算 semver 版本。
get_version_from_tag() {
  local tag="$1"
  local version="${tag#hotpot-v}"
  version="${version#v}"
  echo "$version"
}

# Update Chocolatey package files from the release checksum directory.
# 使用 release 校验和目录更新 Chocolatey 包文件。
main() {
  if [[ $# -lt 2 ]]; then
    echo "Usage: $0 <tag> <sha256_dir>" >&2
    echo "Example: $0 hotpot-v0.3.2 ./sha256" >&2
    exit 1
  fi

  local tag="$1"
  local sha256_dir="$2"
  local version
  version="$(get_version_from_tag "$tag")"

  echo "=== Updating Chocolatey package files for tag: $tag (version: $version) ==="

  local script_dir
  script_dir="$(cd "$(dirname "$0")" && pwd)"
  local project_root
  project_root="$(cd "$script_dir/.." && pwd)"

  local windows_sha_file="$sha256_dir/hotpot-${tag}-windows-x86_64.zip.sha256"
  local sha_windows_x86_64
  sha_windows_x86_64="$(get_hash "$windows_sha_file")"
  echo "  Found Windows x86_64 SHA256: $sha_windows_x86_64"

  local nuspec_file="$project_root/packaging/chocolatey/hotpot.nuspec"
  if [[ -f "$nuspec_file" ]]; then
    echo "Updating Chocolatey nuspec: $nuspec_file"
    sed_i "s|<version>[0-9.]*</version>|<version>${version}</version>|" "$nuspec_file"
    echo "  Chocolatey nuspec updated."
  else
    echo "Error: Chocolatey nuspec not found: $nuspec_file" >&2
    exit 1
  fi

  local choco_install="$project_root/packaging/chocolatey/tools/chocolateyInstall.ps1"
  if [[ -f "$choco_install" ]]; then
    echo "Updating Chocolatey install script: $choco_install"
    sed_i "s/\\\$version[[:space:]]*=[[:space:]]*'[^']*'/\\\$version        = '${version}'/" "$choco_install"
    sed_i "s/\\\$checksum64[[:space:]]*=[[:space:]]*'[^']*'/\\\$checksum64     = '${sha_windows_x86_64}'/" "$choco_install"
    echo "  Chocolatey install script updated."
  else
    echo "Error: Chocolatey install script not found: $choco_install" >&2
    exit 1
  fi

  echo ""
  echo "=== Chocolatey package files updated successfully for ${tag} ==="
}

main "$@"
