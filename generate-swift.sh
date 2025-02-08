#!/usr/bin/env zsh
set -e

FFI_PROJECT="src/intershare_sdk_ffi/Cargo.toml"

# Colors
CYAN="\e[36m"
RED="\e[0;31m"
GREEN="\e[32m"
ENDCOLOR="\e[0m"

function PrintInfo()
{
    echo -e "${CYAN}$1${ENDCOLOR}"
}

function PrintDone()
{
    echo -e "    ${GREEN}Done${ENDCOLOR}"
    echo ""
    echo ""
}

function BuildStaticLibrary()
{
    Target=$1
    PrintInfo "Building for $Target"
    cargo build --manifest-path $FFI_PROJECT --lib --release --target $Target

    PrintDone
}

function GenerateUniffiBindings()
{
    PrintInfo "Generating bindings"
    cargo build --release
    cargo run --bin uniffi-bindgen generate --library target/release/libintershare_sdk_ffi.a --language swift --out-dir "bindings/swift/Sources/InterShareKit"
    # cargo run --bin uniffi-bindgen generate "src/intershare_sdk_ffi/src/intershare_sdk.udl" --language swift --out-dir "bindings/swift/Sources/InterShareSDK"

    pushd bindings/swift
        mv Sources/InterShareKit/*.h .out/headers/
        mv Sources/InterShareKit/*.modulemap .out/headers/module.modulemap
    popd

    PrintDone
}

function CreateUniversalBinary()
{
    Target=$1
    FirstArchitecture=$2
    SecondArchitecture=$3

    PrintInfo "Generating universal binary for $Target"

    if [ -z "$SecondArchitecture" ]
    then
        lipo -create \
          "target/$FirstArchitecture/release/libintershare_sdk_ffi.a" \
          -output "bindings/swift/.out/$Target/libintershare_sdk_ffi.a"
    else
        lipo -create \
          "target/$FirstArchitecture/release/libintershare_sdk_ffi.a" \
          "target/$SecondArchitecture/release/libintershare_sdk_ffi.a" \
          -output "bindings/swift/.out/$Target/libintershare_sdk_ffi.a"
    fi

    strip -x "bindings/swift/.out/$Target/libintershare_sdk_ffi.a"

    PrintDone
}

function GenerateXcFramework()
{
    PrintInfo "Generating xc-framework"

    rm -rf bindings/swift/InterShareSDKFFI.xcframework

    xcodebuild -create-xcframework \
      -library bindings/swift/.out/macos/libintershare_sdk_ffi.a \
      -headers bindings/swift/.out/headers/ \
      -library bindings/swift/.out/ios/libintershare_sdk_ffi.a \
      -headers bindings/swift/.out/headers/ \
      -library bindings/swift/.out/ios-simulator/libintershare_sdk_ffi.a \
      -headers bindings/swift/.out/headers/ \
      -output bindings/swift/InterShareSDKFFI.xcframework

    PrintDone
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
BuildStaticLibrary aarch64-apple-ios

# iOS Simulator
BuildStaticLibrary aarch64-apple-ios-sim
BuildStaticLibrary x86_64-apple-ios

# macOS
BuildStaticLibrary x86_64-apple-darwin
BuildStaticLibrary aarch64-apple-darwin

GenerateUniffiBindings

CreateUniversalBinary "macos" "x86_64-apple-darwin" "aarch64-apple-darwin"
CreateUniversalBinary "ios" "aarch64-apple-ios"
CreateUniversalBinary "ios-simulator" "x86_64-apple-ios" "aarch64-apple-ios-sim"

GenerateXcFramework

#zip -r InterShareSDKFFI.xcframework.zip InterShareSDKFFI.xcframework

rm -rf bindings/swift/.out
