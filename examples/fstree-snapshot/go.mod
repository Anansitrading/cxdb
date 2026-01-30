module github.com/strongdm/cxdb/examples/fstree-snapshot

go 1.22

require (
	github.com/strongdm/ai-cxdb/clients/go v0.0.8
	github.com/vmihailenco/msgpack/v5 v5.4.1
)

// Local development: use the parent repository's client
replace github.com/strongdm/ai-cxdb/clients/go => ../../clients/go

require (
	github.com/klauspost/cpuid/v2 v2.0.12 // indirect
	github.com/vmihailenco/tagparser/v2 v2.0.0 // indirect
	github.com/zeebo/blake3 v0.2.4 // indirect
)
