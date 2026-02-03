// Copyright 2025 StrongDM Inc
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"bytes"
	"crypto/tls"
	"encoding/binary"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/vmihailenco/msgpack/v5"
	"github.com/zeebo/blake3"
)

const (
	msgHello     uint16 = 1
	msgCtxCreate uint16 = 2
	msgCtxFork   uint16 = 3
	msgGetHead   uint16 = 4
	msgAppend    uint16 = 5
	msgGetLast   uint16 = 6
	msgGetBlob   uint16 = 9
	msgError     uint16 = 255
)

const (
	encodingMsgpack uint32 = 1
	compressionNone uint32 = 0
)

type frame struct {
	msgType uint16
	reqID   uint64
	payload []byte
}

func main() {
	if len(os.Args) < 2 {
		usage()
		os.Exit(1)
	}

	switch os.Args[1] {
	case "create-context":
		cmdCreateContext(os.Args[2:])
	case "append":
		cmdAppend(os.Args[2:])
	case "get-last":
		cmdGetLast(os.Args[2:])
	case "publish-registry":
		cmdPublishRegistry(os.Args[2:])
	case "get-typed":
		cmdGetTyped(os.Args[2:])
	case "get-metrics":
		cmdGetMetrics(os.Args[2:])
	default:
		usage()
		os.Exit(1)
	}
}

func usage() {
	fmt.Println("Usage:")
	fmt.Println("")
	fmt.Println("HTTP API Commands:")
	fmt.Println("  publish-registry -http URL -bundle-id ID -file path.json")
	fmt.Println("  get-typed -http URL -context ID [-limit N]")
	fmt.Println("  get-metrics -http URL")
	fmt.Println("")
	fmt.Println("Binary Protocol Commands:")
	fmt.Println("  create-context [-addr host:port] [-base 0]")
	fmt.Println("  append [-addr host:port] -context ID -role ROLE -text TEXT [-type-id ID] [-type-version N] [-parent ID]")
	fmt.Println("  get-last [-addr host:port] -context ID [-limit N]")
	fmt.Println("")
	fmt.Println("Development endpoints:")
	fmt.Println("  HTTP API:        http://localhost:9010")
	fmt.Println("  Binary Protocol: localhost:9009")
}

func cmdCreateContext(args []string) {
	fs := flag.NewFlagSet("create-context", flag.ExitOnError)
	addr := fs.String("addr", "localhost:9009", "server address")
	base := fs.Uint64("base", 0, "base turn id")
	fs.Parse(args)

	conn := mustDial(*addr)
	defer conn.Close()

	payload := make([]byte, 8)
	binary.LittleEndian.PutUint64(payload, *base)
	reqID := uint64(time.Now().UnixNano())
	mustWriteFrame(conn, msgCtxCreate, reqID, payload)

	resp := mustReadFrame(conn)
	if resp.msgType == msgError {
		fatalError(resp.payload)
	}

	if len(resp.payload) < 20 {
		fmt.Println("invalid response")
		os.Exit(1)
	}

	contextID := binary.LittleEndian.Uint64(resp.payload[0:8])
	headTurnID := binary.LittleEndian.Uint64(resp.payload[8:16])
	headDepth := binary.LittleEndian.Uint32(resp.payload[16:20])

	fmt.Printf("context_id=%d head_turn_id=%d head_depth=%d\n", contextID, headTurnID, headDepth)
}

func cmdAppend(args []string) {
	fs := flag.NewFlagSet("append", flag.ExitOnError)
	addr := fs.String("addr", "localhost:9009", "server address")
	contextID := fs.Uint64("context", 0, "context id")
	parentID := fs.Uint64("parent", 0, "parent turn id (optional)")
	role := fs.String("role", "user", "role value")
	text := fs.String("text", "", "text value")
	typeID := fs.String("type-id", "com.yourorg.ai.MessageTurn", "declared type id")
	typeVersion := fs.Uint("type-version", 1, "declared type version")
	fs.Parse(args)

	if *contextID == 0 {
		fmt.Println("context is required")
		os.Exit(1)
	}

	payloadBytes := encodeMessageTurn(*role, *text)
	hash := blake3.Sum256(payloadBytes)

	payload := &bytes.Buffer{}
	_ = binary.Write(payload, binary.LittleEndian, *contextID)
	_ = binary.Write(payload, binary.LittleEndian, *parentID)

	_ = binary.Write(payload, binary.LittleEndian, uint32(len(*typeID)))
	payload.WriteString(*typeID)
	_ = binary.Write(payload, binary.LittleEndian, uint32(*typeVersion))

	_ = binary.Write(payload, binary.LittleEndian, uint32(encodingMsgpack))
	_ = binary.Write(payload, binary.LittleEndian, uint32(compressionNone))
	_ = binary.Write(payload, binary.LittleEndian, uint32(len(payloadBytes)))
	payload.Write(hash[:])

	_ = binary.Write(payload, binary.LittleEndian, uint32(len(payloadBytes)))
	payload.Write(payloadBytes)

	_ = binary.Write(payload, binary.LittleEndian, uint32(0)) // idempotency key len

	conn := mustDial(*addr)
	defer conn.Close()

	reqID := uint64(time.Now().UnixNano())
	mustWriteFrame(conn, msgAppend, reqID, payload.Bytes())

	resp := mustReadFrame(conn)
	if resp.msgType == msgError {
		fatalError(resp.payload)
	}

	if len(resp.payload) < 20 {
		fmt.Println("invalid response")
		os.Exit(1)
	}

	newTurnID := binary.LittleEndian.Uint64(resp.payload[8:16])
	newDepth := binary.LittleEndian.Uint32(resp.payload[16:20])
	fmt.Printf("turn_id=%d depth=%d\n", newTurnID, newDepth)
}

func cmdGetLast(args []string) {
	fs := flag.NewFlagSet("get-last", flag.ExitOnError)
	addr := fs.String("addr", "localhost:9009", "server address")
	contextID := fs.Uint64("context", 0, "context id")
	limit := fs.Uint("limit", 10, "limit")
	fs.Parse(args)

	if *contextID == 0 {
		fmt.Println("context is required")
		os.Exit(1)
	}

	payload := &bytes.Buffer{}
	_ = binary.Write(payload, binary.LittleEndian, *contextID)
	_ = binary.Write(payload, binary.LittleEndian, uint32(*limit))
	_ = binary.Write(payload, binary.LittleEndian, uint32(1))

	conn := mustDial(*addr)
	defer conn.Close()

	reqID := uint64(time.Now().UnixNano())
	mustWriteFrame(conn, msgGetLast, reqID, payload.Bytes())

	resp := mustReadFrame(conn)
	if resp.msgType == msgError {
		fatalError(resp.payload)
	}

	cursor := bytes.NewReader(resp.payload)
	var count uint32
	_ = binary.Read(cursor, binary.LittleEndian, &count)

	for i := 0; i < int(count); i++ {
		var turnID, parentID uint64
		var depth uint32
		_ = binary.Read(cursor, binary.LittleEndian, &turnID)
		_ = binary.Read(cursor, binary.LittleEndian, &parentID)
		_ = binary.Read(cursor, binary.LittleEndian, &depth)

		var typeLen uint32
		_ = binary.Read(cursor, binary.LittleEndian, &typeLen)
		typeBytes := make([]byte, typeLen)
		_, _ = cursor.Read(typeBytes)
		var typeVersion uint32
		_ = binary.Read(cursor, binary.LittleEndian, &typeVersion)
		var encoding uint32
		_ = binary.Read(cursor, binary.LittleEndian, &encoding)
		var compression uint32
		_ = binary.Read(cursor, binary.LittleEndian, &compression)
		var uncompressedLen uint32
		_ = binary.Read(cursor, binary.LittleEndian, &uncompressedLen)
		var hash [32]byte
		_, _ = cursor.Read(hash[:])
		var payloadLen uint32
		_ = binary.Read(cursor, binary.LittleEndian, &payloadLen)
		payload := make([]byte, payloadLen)
		_, _ = cursor.Read(payload)

		fmt.Printf("turn_id=%d depth=%d type=%s v%d len=%d\n", turnID, depth, string(typeBytes), typeVersion, payloadLen)
	}
}

func cmdPublishRegistry(args []string) {
	fs := flag.NewFlagSet("publish-registry", flag.ExitOnError)
	baseURL := fs.String("http", "http://localhost:9010", "http base url")
	bundleID := fs.String("bundle-id", "", "bundle id (must match JSON)")
	filePath := fs.String("file", "", "path to registry bundle JSON")
	fs.Parse(args)

	if *bundleID == "" || *filePath == "" {
		fmt.Println("bundle-id and file are required")
		os.Exit(1)
	}

	body, err := os.ReadFile(filepath.Clean(*filePath))
	if err != nil {
		fmt.Println("read file error:", err)
		os.Exit(1)
	}

	escaped := url.PathEscape(*bundleID)
	endpoint := fmt.Sprintf("%s/v1/registry/bundles/%s", *baseURL, escaped)
	req, err := http.NewRequest(http.MethodPut, endpoint, bytes.NewReader(body))
	if err != nil {
		fmt.Println("http request error:", err)
		os.Exit(1)
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		fmt.Println("http error:", err)
		os.Exit(1)
	}
	defer resp.Body.Close()

	respBody, _ := io.ReadAll(resp.Body)
	if resp.StatusCode >= 400 {
		fmt.Printf("error %d: %s\n", resp.StatusCode, string(respBody))
		os.Exit(1)
	}

	fmt.Printf("status=%d body=%s\n", resp.StatusCode, bytes.TrimSpace(respBody))
}

func cmdGetTyped(args []string) {
	fs := flag.NewFlagSet("get-typed", flag.ExitOnError)
	baseURL := fs.String("http", "http://localhost:9010", "http base url")
	contextID := fs.Uint64("context", 0, "context id")
	limit := fs.Uint("limit", 10, "limit")
	fs.Parse(args)

	if *contextID == 0 {
		fmt.Println("context is required")
		os.Exit(1)
	}

	endpoint := fmt.Sprintf("%s/v1/contexts/%d/turns?view=typed&type_hint_mode=inherit&limit=%d", *baseURL, *contextID, *limit)
	resp, err := http.Get(endpoint)
	if err != nil {
		fmt.Println("http error:", err)
		os.Exit(1)
	}
	defer resp.Body.Close()

	body, _ := io.ReadAll(resp.Body)
	if resp.StatusCode >= 400 {
		fmt.Printf("error %d: %s\n", resp.StatusCode, string(body))
		os.Exit(1)
	}

	var parsed any
	if err := json.Unmarshal(body, &parsed); err != nil {
		fmt.Println("json decode error:", err)
		os.Exit(1)
	}
	pretty, _ := json.MarshalIndent(parsed, "", "  ")
	fmt.Println(string(pretty))
}

func cmdGetMetrics(args []string) {
	fs := flag.NewFlagSet("get-metrics", flag.ExitOnError)
	baseURL := fs.String("http", "http://localhost:9010", "http base url")
	fs.Parse(args)

	endpoint := fmt.Sprintf("%s/v1/metrics", *baseURL)
	resp, err := http.Get(endpoint)
	if err != nil {
		fmt.Println("http error:", err)
		os.Exit(1)
	}
	defer resp.Body.Close()

	body, _ := io.ReadAll(resp.Body)
	if resp.StatusCode >= 400 {
		fmt.Printf("error %d: %s\n", resp.StatusCode, string(body))
		os.Exit(1)
	}

	var parsed any
	if err := json.Unmarshal(body, &parsed); err != nil {
		fmt.Println("json decode error:", err)
		os.Exit(1)
	}
	pretty, _ := json.MarshalIndent(parsed, "", "  ")
	fmt.Println(string(pretty))
}

func encodeMessageTurn(role, text string) []byte {
	payload := map[uint64]interface{}{
		1: role,
		2: text,
	}

	buf := &bytes.Buffer{}
	enc := msgpack.NewEncoder(buf)
	enc.SetSortMapKeys(true)
	_ = enc.Encode(payload)
	return buf.Bytes()
}

func mustDial(addr string) net.Conn {
	// Use TLS for port 443 (production), plain TCP for other ports (development)
	if strings.HasSuffix(addr, ":443") {
		conn, err := tls.Dial("tcp", addr, &tls.Config{})
		if err != nil {
			fmt.Println("tls dial error:", err)
			os.Exit(1)
		}
		return conn
	}
	conn, err := net.Dial("tcp", addr)
	if err != nil {
		fmt.Println("dial error:", err)
		os.Exit(1)
	}
	return conn
}

func mustWriteFrame(conn net.Conn, msgType uint16, reqID uint64, payload []byte) {
	header := &bytes.Buffer{}
	_ = binary.Write(header, binary.LittleEndian, uint32(len(payload)))
	_ = binary.Write(header, binary.LittleEndian, msgType)
	_ = binary.Write(header, binary.LittleEndian, uint16(0))
	_ = binary.Write(header, binary.LittleEndian, reqID)
	_, _ = conn.Write(append(header.Bytes(), payload...))
}

func mustReadFrame(conn net.Conn) frame {
	header := make([]byte, 16)
	_, err := readFull(conn, header)
	if err != nil {
		fmt.Println("read error:", err)
		os.Exit(1)
	}

	length := binary.LittleEndian.Uint32(header[0:4])
	msgType := binary.LittleEndian.Uint16(header[4:6])
	reqID := binary.LittleEndian.Uint64(header[8:16])

	payload := make([]byte, length)
	_, err = readFull(conn, payload)
	if err != nil {
		fmt.Println("read payload error:", err)
		os.Exit(1)
	}

	return frame{msgType: msgType, reqID: reqID, payload: payload}
}

func readFull(conn net.Conn, buf []byte) (int, error) {
	total := 0
	for total < len(buf) {
		n, err := conn.Read(buf[total:])
		if err != nil {
			return total, err
		}
		total += n
	}
	return total, nil
}

func fatalError(payload []byte) {
	if len(payload) < 8 {
		fmt.Println("error from server")
		os.Exit(1)
	}
	code := binary.LittleEndian.Uint32(payload[0:4])
	detailLen := binary.LittleEndian.Uint32(payload[4:8])
	detail := ""
	if int(detailLen) <= len(payload[8:]) {
		detail = string(payload[8 : 8+detailLen])
	}
	fmt.Printf("error %d: %s\n", code, detail)
	os.Exit(1)
}
