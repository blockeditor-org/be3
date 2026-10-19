package main

import (
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"crypto/x509/pkix"
	"fmt"
	"math/big"
	"os"
	"strconv"
	"time"

	"software.sslmate.com/src/go-pkcs12"
)

func init() {
	if len(os.Args) != 4 || os.Args[1] != "--generate-key" {
		return
	}
	bits, err := strconv.Atoi(os.Args[3])
	if err == nil {
		err = generateKey(os.Args[2], bits)
	}
	if err != nil {
		fmt.Fprintln(os.Stderr, "key generation:", err)
		os.Exit(1)
	}
	os.Exit(0)
}

func generateKey(path string, bits int) error {
	key, err := rsa.GenerateKey(rand.Reader, bits)
	if err != nil {
		return err
	}
	serial, err := rand.Int(rand.Reader, new(big.Int).Lsh(big.NewInt(1), 128))
	if err != nil {
		return err
	}
	now := time.Now()
	cert := &x509.Certificate{
		SerialNumber: serial,
		Subject:      pkix.Name{CommonName: "Block Android signing"},
		NotBefore:    now.Add(-24 * time.Hour),
		NotAfter:     now.AddDate(100, 0, 0),
		KeyUsage:     x509.KeyUsageDigitalSignature,
	}
	der, err := x509.CreateCertificate(rand.Reader, cert, cert, &key.PublicKey, key)
	if err != nil {
		return err
	}
	parsed, err := x509.ParseCertificate(der)
	if err != nil {
		return err
	}
	data, err := pkcs12.Modern.Encode(key, parsed, nil, "android")
	if err != nil {
		return err
	}
	return os.WriteFile(path, data, 0600)
}
