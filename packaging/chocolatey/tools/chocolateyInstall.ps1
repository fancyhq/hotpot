# Chocolatey install script for hotpot.
# Chocolatey 安装脚本 for hotpot.
#
# Downloads the Windows x86_64 release zip from GitHub Releases,
# verifies the SHA256 checksum, and installs hotpot.exe.
# 从 GitHub Releases 下载 Windows x86_64 发布 zip，
# 验证 SHA256 校验和，然后安装 hotpot.exe。
#
# Placeholders are replaced by scripts/update-release-package-manifests.sh
# during the release workflow.
# 占位符由 release workflow 中的 scripts/update-release-package-manifests.sh 替换。

$ErrorActionPreference = 'Stop'

$packageName    = 'hotpot'
$version        = '0.3.2'
$tag            = "hotpot-v$version"
$url64          = "https://github.com/fancyhq/hotpot/releases/download/$tag/hotpot-$tag-windows-x86_64.zip"
$checksum64     = 'PLACEHOLDER_WINDOWS_X86_64_SHA256'

$toolsDir = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"

# Download, verify checksum, and extract the Windows release zip.
# 下载、校验并将其解压。
Install-ChocolateyZipPackage `
  -PackageName "$packageName" `
  -Url64bit "$url64" `
  -UnzipLocation "$toolsDir" `
  -Checksum64 "$checksum64" `
  -ChecksumType64 'sha256'

# Install-ChocolateyZipPackage automatically creates shims for
# any executables found in the extracted files, so hotpot.exe
# will be available on PATH without an explicit shim command.
# Install-ChocolateyZipPackage 会自动为解压出的可执行文件创建 shim，
# 因此无需额外命令即可使 hotpot.exe 在 PATH 中可用。
