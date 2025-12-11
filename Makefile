# =============================================================================
# Drone WebSocket Target Tracking System - Production Makefile
# =============================================================================

SHELL := /bin/bash
.DEFAULT_GOAL := help

# -----------------------------------------------------------------------------
# Configuration
# -----------------------------------------------------------------------------

PROJECT_NAME := drone-ws-target-tracking
CARGO := cargo
DOCKER := docker
DOCKER_COMPOSE := docker compose

# Docker image tags
REGISTRY ?= 
IMAGE_TAG ?= latest
SERVER_IMAGE := $(if $(REGISTRY),$(REGISTRY)/)$(PROJECT_NAME)-server:$(IMAGE_TAG)
CLIENT_IMAGE := $(if $(REGISTRY),$(REGISTRY)/)$(PROJECT_NAME)-client:$(IMAGE_TAG)

# Build configuration
RELEASE_FLAGS := --release
CARGO_BUILD_JOBS ?= $(shell nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)

# Certificate paths
CERT_DIR := certificates
CERT_FILE := $(CERT_DIR)/server.pem
KEY_FILE := $(CERT_DIR)/server-key.pem

# -----------------------------------------------------------------------------
# Colors for pretty output
# -----------------------------------------------------------------------------

CYAN := \033[36m
GREEN := \033[32m
YELLOW := \033[33m
RED := \033[31m
RESET := \033[0m

# -----------------------------------------------------------------------------
# Help
# -----------------------------------------------------------------------------

.PHONY: help
help: ## Show this help message
	@echo -e "$(CYAN)$(PROJECT_NAME)$(RESET) - Production Build System"
	@echo ""
	@echo -e "$(YELLOW)Usage:$(RESET)"
	@echo "  make <target>"
	@echo ""
	@echo -e "$(YELLOW)Targets:$(RESET)"
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z_-]+:.*##/ { printf "  $(CYAN)%-20s$(RESET) %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

# -----------------------------------------------------------------------------
# Development Builds
# -----------------------------------------------------------------------------

.PHONY: build
build: ## Build all workspace members (debug)
	@echo -e "$(CYAN)Building workspace (debug)...$(RESET)"
	$(CARGO) build -j$(CARGO_BUILD_JOBS)

.PHONY: release
release: ## Build all workspace members (release)
	@echo -e "$(CYAN)Building workspace (release)...$(RESET)"
	$(CARGO) build $(RELEASE_FLAGS) -j$(CARGO_BUILD_JOBS)

.PHONY: server
server: ## Build ws-server (release)
	@echo -e "$(CYAN)Building ws-server...$(RESET)"
	$(CARGO) build -p ws-server $(RELEASE_FLAGS)

.PHONY: client
client: ## Build ws-client (release)
	@echo -e "$(CYAN)Building ws-client...$(RESET)"
	$(CARGO) build -p ws-client $(RELEASE_FLAGS)

# -----------------------------------------------------------------------------
# Testing & Quality
# -----------------------------------------------------------------------------

.PHONY: test
test: ## Run all tests
	@echo -e "$(CYAN)Running tests...$(RESET)"
	$(CARGO) test --workspace

.PHONY: test-verbose
test-verbose: ## Run tests with output
	@echo -e "$(CYAN)Running tests (verbose)...$(RESET)"
	$(CARGO) test --workspace -- --nocapture

.PHONY: clippy
clippy: ## Run clippy lints
	@echo -e "$(CYAN)Running clippy...$(RESET)"
	$(CARGO) clippy --workspace --all-targets -- -D warnings

.PHONY: fmt
fmt: ## Format code
	@echo -e "$(CYAN)Formatting code...$(RESET)"
	$(CARGO) fmt --all

.PHONY: fmt-check
fmt-check: ## Check code formatting
	@echo -e "$(CYAN)Checking format...$(RESET)"
	$(CARGO) fmt --all -- --check

.PHONY: audit
audit: ## Security audit dependencies
	@echo -e "$(CYAN)Auditing dependencies...$(RESET)"
	$(CARGO) audit

.PHONY: check
check: fmt-check clippy test ## Run all checks (format, lint, test)
	@echo -e "$(GREEN)All checks passed!$(RESET)"

# -----------------------------------------------------------------------------
# Certificates
# -----------------------------------------------------------------------------

.PHONY: certs
certs: $(CERT_FILE) ## Generate TLS certificates with mkcert

$(CERT_FILE): | $(CERT_DIR)
	@echo -e "$(CYAN)Generating TLS certificates...$(RESET)"
	@command -v mkcert >/dev/null 2>&1 || { echo -e "$(RED)Error: mkcert not found. Install with: brew install mkcert$(RESET)"; exit 1; }
	mkcert -install
	mkcert -cert-file $(CERT_FILE) -key-file $(KEY_FILE) localhost 127.0.0.1 ::1
	@echo -e "$(GREEN)Certificates generated in $(CERT_DIR)/$(RESET)"

$(CERT_DIR):
	mkdir -p $(CERT_DIR)

.PHONY: certs-clean
certs-clean: ## Remove generated certificates
	@echo -e "$(YELLOW)Removing certificates...$(RESET)"
	rm -f $(CERT_FILE) $(KEY_FILE)

# -----------------------------------------------------------------------------
# Docker Builds
# -----------------------------------------------------------------------------

.PHONY: docker-build
docker-build: docker-server docker-client ## Build all Docker images

.PHONY: docker-server
docker-server: ## Build ws-server Docker image
	@echo -e "$(CYAN)Building server image: $(SERVER_IMAGE)$(RESET)"
	$(DOCKER) build -t $(SERVER_IMAGE) -f ws-server/Dockerfile .

.PHONY: docker-client
docker-client: ## Build ws-client Docker image
	@echo -e "$(CYAN)Building client image: $(CLIENT_IMAGE)$(RESET)"
	$(DOCKER) build -t $(CLIENT_IMAGE) -f ws-client/Dockerfile .

.PHONY: docker-push
docker-push: ## Push images to registry (requires REGISTRY env var)
	@if [ -z "$(REGISTRY)" ]; then echo -e "$(RED)Error: REGISTRY not set$(RESET)"; exit 1; fi
	@echo -e "$(CYAN)Pushing images to $(REGISTRY)...$(RESET)"
	$(DOCKER) push $(SERVER_IMAGE)
	$(DOCKER) push $(CLIENT_IMAGE)

# -----------------------------------------------------------------------------
# Docker Compose
# -----------------------------------------------------------------------------

.PHONY: up
up: certs ## Start all services with docker-compose
	@echo -e "$(CYAN)Starting services...$(RESET)"
	$(DOCKER_COMPOSE) up -d

.PHONY: up-build
up-build: certs ## Build and start all services
	@echo -e "$(CYAN)Building and starting services...$(RESET)"
	$(DOCKER_COMPOSE) up -d --build

.PHONY: down
down: ## Stop all services
	@echo -e "$(YELLOW)Stopping services...$(RESET)"
	$(DOCKER_COMPOSE) down

.PHONY: logs
logs: ## Tail service logs
	$(DOCKER_COMPOSE) logs -f

.PHONY: logs-server
logs-server: ## Tail server logs
	$(DOCKER_COMPOSE) logs -f ws-server

.PHONY: logs-client
logs-client: ## Tail client logs
	$(DOCKER_COMPOSE) logs -f ws-client

.PHONY: ps
ps: ## Show running containers
	$(DOCKER_COMPOSE) ps

# -----------------------------------------------------------------------------
# Local Run
# -----------------------------------------------------------------------------

.PHONY: run-server
run-server: certs release ## Run server locally
	@echo -e "$(CYAN)Starting server...$(RESET)"
	RUST_LOG=info ./target/release/ws-server

.PHONY: run-client
run-client: release ## Run client locally (interactive)
	@echo -e "$(CYAN)Starting client (interactive)...$(RESET)"
	RUST_LOG=info ./target/release/ws-client --interactive

.PHONY: run-client-once
run-client-once: release ## Run client locally (single message)
	@echo -e "$(CYAN)Starting client (single message)...$(RESET)"
	RUST_LOG=info ./target/release/ws-client

# -----------------------------------------------------------------------------
# Cleanup
# -----------------------------------------------------------------------------

.PHONY: clean
clean: ## Clean build artifacts
	@echo -e "$(YELLOW)Cleaning build artifacts...$(RESET)"
	$(CARGO) clean

.PHONY: clean-docker
clean-docker: ## Remove Docker images
	@echo -e "$(YELLOW)Removing Docker images...$(RESET)"
	-$(DOCKER) rmi $(SERVER_IMAGE) $(CLIENT_IMAGE) 2>/dev/null

.PHONY: clean-all
clean-all: clean clean-docker certs-clean ## Clean everything
	@echo -e "$(GREEN)All clean!$(RESET)"

# -----------------------------------------------------------------------------
# CI Targets
# -----------------------------------------------------------------------------

.PHONY: ci
ci: fmt-check clippy test ## CI pipeline (no docker)
	@echo -e "$(GREEN)CI checks passed!$(RESET)"

.PHONY: ci-full
ci-full: ci docker-build ## Full CI pipeline with Docker builds
	@echo -e "$(GREEN)Full CI pipeline passed!$(RESET)"
