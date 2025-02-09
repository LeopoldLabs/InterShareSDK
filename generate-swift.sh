#!/usr/bin/env zsh
set -e

FFI_PROJECT="src/intershare_sdk/Cargo.toml"

# Colors
CYAN="\e[36m"
RED="\e[0;31m"
GREEN="\e[32m"
ENDCOLOR="\e[0m"

function printInfo()
{
    echo -e "${CYAN}$1${ENDCOLOR}"
}

function printDone()
{
    echo -e "    ${GREEN}Done${ENDCOLOR}"
    echo ""
    echo ""
}

function buildStaticLibrary()
{
    target=$1
    printInfo "Building for $target"
    cargo build --manifest-path $FFI_PROJECT --lib --release --target $target

    printDone
}

function generateUniffiBindings()
{
    printInfo "Generating bindings"
    cargo build --release
    cargo run --bin uniffi-bindgen generate --library target/release/libintershare_sdk.a --language swift --out-dir "bindings/swift/Sources/InterShareKit"
    # cargo run --bin uniffi-bindgen generate "src/intershare_sdk_ffi/src/intershare_sdk.udl" --language swift --out-dir "bindings/swift/Sources/InterShareSDK"

    pushd bindings/swift
        mv Sources/InterShareKit/*.h .out/headers/
        mv Sources/InterShareKit/*.modulemap .out/headers/module.modulemap
    popd

    printDone
}

function createUniversalBinary()
{
    target=$1
    firstArchitecture=$2
    secondArchitecture=$3

    printInfo "Generating universal binary for $target"

    if [ -z "$secondArchitecture" ]
    then
        lipo -create \
          "target/$firstArchitecture/release/libintershare_sdk.a" \
          -output "bindings/swift/.out/$target/libintershare_sdk.a"
    else
        lipo -create \
          "target/$firstArchitecture/release/libintershare_sdk.a" \
          "target/$secondArchitecture/release/libintershare_sdk.a" \
          -output "bindings/swift/.out/$target/libintershare_sdk.a"
    fi

    # strip -x "bindings/swift/.out/$Target/libintershare_sdk.a"

    printDone
}

function generateXcFramework()
{
    printInfo "Generating xc-framework"

    rm -rf bindings/swift/InterShareSDKFFI.xcframework

    xcodebuild -create-xcframework \
      -library bindings/swift/.out/macos/libintershare_sdk.a \
      -headers bindings/swift/.out/headers/ \
      -library bindings/swift/.out/ios/libintershare_sdk.a \
      -headers bindings/swift/.out/headers/ \
      -library bindings/swift/.out/ios-simulator/libintershare_sdk.a \
      -headers bindings/swift/.out/headers/ \
      -output bindings/swift/InterShareSDKFFI.xcframework

    printDone
}



# ======= main =======

rm -rf bindings/swift/.out
mkdir bindings/swift/.out
mkdir bindings/swift/.out/headers
mkdir bindings/swift/.out/macos
mkdir bindings/swift/.out/ios
mkdir bindings/swift/.out/ios-simulator

export MACOSX_DEPLOYMENT_TARGET=12.0

# iOS
buildStaticLibrary aarch64-apple-ios

# iOS Simulator
buildStaticLibrary aarch64-apple-ios-sim
buildStaticLibrary x86_64-apple-ios

# macOS
buildStaticLibrary x86_64-apple-darwin
buildStaticLibrary aarch64-apple-darwin

generateUniffiBindings

createUniversalBinary "macos" "x86_64-apple-darwin" "aarch64-apple-darwin"
createUniversalBinary "ios" "aarch64-apple-ios"
createUniversalBinary "ios-simulator" "x86_64-apple-ios" "aarch64-apple-ios-sim"

generateXcFramework

#zip -r InterShareSDKFFI.xcframework.zip InterShareSDKFFI.xcframework

rm -rf bindings/swift/.out
