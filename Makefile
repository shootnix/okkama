# Сборка и установка Okkama (Rust-реализация).
#
#     make                 # = make build
#     make install         # сборка и установка в ~/.local (PREFIX=/opt/okkama make install),
#                          # вместе с расширением VS Code, если есть VS Code и npm
#     make uninstall
#     make test            # тесты Rust
#     make gate            # ворота релиза в контейнере: тесты, conformance, std (make gate-local — без контейнера)
#     make pgo             # release-сборка с PGO (нужен `rustup component add llvm-tools`)
#     make install-pgo     # PGO-сборка и установка
#     make dist            # пакеты этой платформы в dist/ (tar.gz + .pkg/.deb)
#     make vscode          # расширение VS Code (.vsix) и его установка в VS Code
#
# `build` собирает okkama, sparrow и blade; криптография встроена в Sparrow.

PREFIX ?= $(HOME)/.local
CARGO  ?= cargo

RS      := okkama-rs
VSIX    := vscode-okkama/okkama-vscode.vsix

.PHONY: all build pgo install install-pgo uninstall test gate gate-local dist vscode editor clean

all: build

build:
	cd $(RS) && $(CARGO) build --release -p okkama-cli -p sparrow-cli -p blade-cli

install: build
	./install.sh --prefix "$(PREFIX)" --no-build
	@$(MAKE) --no-print-directory editor

# Оптимизация по профилю (tools/pgo/build.sh): на 6–18% быстрее обычной сборки.
pgo:
	sh $(RS)/tools/pgo/build.sh

install-pgo: pgo
	./install.sh --prefix "$(PREFIX)" --no-build
	@$(MAKE) --no-print-directory editor

uninstall:
	./install.sh --prefix "$(PREFIX)" --uninstall

test:
	cd $(RS) && $(CARGO) test --release --workspace

gate:
	./tools/gate/docker.sh

gate-local:
	./tools/gate/gate.sh

dist:
	./tools/package.sh --installer

$(VSIX): vscode-okkama/package.json vscode-okkama/extension.js vscode-okkama/syntaxes/okkama.tmLanguage.json vscode-okkama/language-configuration.json
	cd vscode-okkama && npm install --omit=dev --no-audit --no-fund
	cd vscode-okkama && npx --yes @vscode/vsce@3 package --skip-license --allow-missing-repository -o okkama-vscode.vsix < /dev/null

vscode: $(VSIX)
	@code="$$(command -v code || echo '/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code')"; \
	if [ -x "$$code" ]; then "$$code" --install-extension $(VSIX) --force \
		&& echo "==> reload every open VS Code window: Cmd+Shift+P, Developer: Reload Window"; \
	else echo "VS Code CLI not found: install $(VSIX) via Extensions: Install from VSIX"; fi

# The editor follows the language: an install also refreshes the VS Code
# extension when VS Code and npm are there, and never fails because of it.
editor:
	@code="$$(command -v code || echo '/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code')"; \
	if [ -x "$$code" ] && command -v npm >/dev/null 2>&1; then \
		$(MAKE) --no-print-directory vscode || echo "==> the VS Code extension was not installed; run make vscode"; \
	else echo "==> VS Code or npm not found: the editor extension is skipped (make vscode)"; fi

clean:
	cd $(RS) && $(CARGO) clean
	rm -f $(VSIX)
