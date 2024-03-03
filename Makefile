.PHONY: build-hive-thrift clean

build-hive-thrift:
	thrift -out hive/src/service/rpc/thrift --gen rs hive/thrift/TCLIService.thrift

clean:
	cargo clean
