#!/bin/bash

set -e

if [ -z "$1" ]; then
  echo "usage: ./release.sh <version> (example: ./release.sh 1.0.1)"
  exit 1
fi

VERSION=$1
TAG="v$VERSION"

# Update version in PKGBUILD
sed -i "s/^pkgver=.*/pkgver=$VERSION/" PKGBUILD

# Update .SRCINFO
makepkg --printsrcinfo > .SRCINFO

# Commit and tag
git add PKGBUILD .SRCINFO
git commit -m "release $TAG"
git tag "$TAG"

# Push to GitHub 
git push origin master --tags

# Push to AUR with verification
echo "Pushing to AUR..."
git show HEAD:PKGBUILD > /dev/null 2>&1 || {
  echo "Error: PKGBUILD not found in HEAD commit!"
  exit 1
}
echo "PKGBUILD found in commit, pushing to AUR..."
git push aur master --tags

echo "✅ released $TAG to GitHub and AUR"
