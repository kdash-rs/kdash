VERSION  := latest
IMG_NAME := deepu105/kdash
IMAGE    := ${IMG_NAME}:${VERSION}

default: run

 ## Run all tests - `cargo install cargo-tarpaulin`
test:  
	@cargo check && cargo tarpaulin

## Builds the app for current os-arch
build:  
	@make test && cargo clean && cargo build --release

## Runs the app
run:  
	@CARGO_INCREMENTAL=1 cargo fmt && make lint && cargo run

## Run clippy
lint:  
	@find . | grep '\.\/src\/.*\.rs$$' | xargs touch && cargo clippy --all-targets --workspace

## Force Run clippy
lint-force:  
	@cargo clean && cargo clippy

## Fix lint
lint-fix:  
	@cargo fix

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