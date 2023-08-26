VERSION  := latest
IMG_NAME := deepu105/kdash
IMAGE    := ${IMG_NAME}:${VERSION}

default: run

 ## Run all tests
test:  
	@cargo test

 ## Run all tests with coverage- `cargo install cargo-tarpaulin`
test-cov:  
	@cargo tarpaulin

## Builds the app for current os-arch
build:  
	@make test && cargo build --release

## Runs the app
run:  
	@CARGO_INCREMENTAL=1 cargo fmt && make lint && cargo run

## Run clippy
lint:  
	cargo clippy --all --all-features --all-targets --workspace -- -D warnings

## Fix lint
lint-fix:  
	@cargo fix --allow-staged

## Run format
fmt:  
	@cargo fmt

## Build a Docker Image
docker:    
	@DOCKER_BUILDKIT=1 docker build --rm -t ${IMAGE} .

## Run Docker Image locally
docker-run:    
	@docker run --rm ${IMAGE}

## Analyse for unsafe usage - `cargo install cargo-geiger`
analyse:  
	@cargo geiger

## Release tag
release:
	@git tag -a ${V} -m "Release ${V}" && git push origin ${V}

## Delete tag
delete-tag:
	@git tag -d ${V} && git push --delete origin ${V}