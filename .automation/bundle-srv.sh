#!/bin/bash

tools="beetle-cli beetle-web beetle-registrar beetle-renderer"
bundle_name=$1
bundle_root=$2
target=$3

if [ -z "$bundle_root" ] || [ -z "$bundle_root" ]; then
  echo "must provide <tarball-name> <output-path> <cargo-target> for artifact"
  exit 1
fi

if [ -f $bundle_name ]; then
  echo "[$0] $bundle_name already exists"
  exit 1
fi

if [ -f $bundle_root ] || [ -d $bundle_root ]; then
  echo "[$0] $bundle_root already exists"
  exit 1
fi

mkdir -p $bundle_root/bin

for tool in $tools; do
  if [ -z "$target" ]; then
    tool_path="src/beetle-srv/target/release/$tool"

    if [ ! -f $tool_path ]; then
      echo "[$0] unable to find '$tool' at $tool_path"
      continue
    fi

    cp -v $tool_path $bundle_root/bin/$tool
  else
    tool_path="src/beetle-srv/target/$target/release/$tool"

    if [ ! -f $tool_path ]; then
      echo "[$0] unable to find '$tool' (at $tool_path)"
      continue
    fi

    cp -v $tool_path $bundle_root/bin/$tool
  fi
done

tar cvzf $bundle_name $bundle_root
