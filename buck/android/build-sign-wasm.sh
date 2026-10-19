set -eu
go="$1" source="$2" keystore="$3" pkcs12="$4" crypto="$5" out="$6" keygen="$7"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch/src/github.com/agusibrahim" "$scratch/src/github.com/pavlo-v-chernykh/keystore-go" "$scratch/src/software.sslmate.com/src" "$scratch/src/golang.org/x"
cp -R "$source" "$scratch/src/github.com/agusibrahim/apksig-go"
cp -R "$keystore" "$scratch/src/github.com/pavlo-v-chernykh/keystore-go/v4"
cp -R "$pkcs12" "$scratch/src/software.sslmate.com/src/go-pkcs12"
cp -R "$crypto" "$scratch/src/golang.org/x/crypto"
cp "$keygen" "$scratch/src/github.com/agusibrahim/apksig-go/cmd/apksign/be3_keygen.go"
GOPATH="$scratch" GOCACHE="$scratch/cache" GO111MODULE=off GOPROXY=off GOTOOLCHAIN=local GOOS=wasip1 GOARCH=wasm CGO_ENABLED=0 \
    "$go/bin/go" build -trimpath -ldflags='-s -w' -o "$out" github.com/agusibrahim/apksig-go/cmd/apksign
