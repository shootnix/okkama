# Сборка и установка Okkama (Rust-реализация).
#
#     make                 # = make build
#     make install         # сборка и установка в ~/.local (PREFIX=/opt/okkama make install)
#     make uninstall
#     make test            # тесты Rust
#     make dist            # пакеты этой платформы в dist/ (tar.gz + .pkg/.deb)
#     make vscode          # расширение VS Code (.vsix) и его установка в VS Code
#
# `build` сначала пересобирает Java-рантайм okkama.rt (если его исходники
# новее классов и есть javac), затем cargo собирает okkama, okkama-native,
# sparrow (VM байткода .spb) и blade (менеджер проектов).
# Классы рантайма встраиваются в сам okkama.

PREFIX ?= $(HOME)/.local
CARGO  ?= cargo

RS      := okkama-rs
RUNTIME := $(RS)/crates/okkama-compiler/runtime/jvm
STAMP   := $(RUNTIME)/classes/.built
JAVA    := $(wildcard $(RUNTIME)/src/okkama/rt/*.java $(RUNTIME)/src22/okkama/rt/*.java)
VSIX    := vscode-okkama/okkama-vscode.vsix

.PHONY: all build runtime install uninstall test dist vscode clean

all: build

runtime: $(STAMP)

$(STAMP): $(JAVA)
	@if command -v javac >/dev/null 2>&1; then \
		echo "==> rebuilding the JVM runtime (okkama.rt)"; \
		log="$$(mktemp)"; \
		bash $(RS)/tools/jvm-runtime/build.sh > "$$log" 2>&1 || { cat "$$log"; rm -f "$$log"; exit 1; }; \
		rm -f "$$log"; \
	else \
		echo "==> javac not found: using the committed JVM runtime classes"; \
	fi
	@touch $@

build: runtime
	cd $(RS) && $(CARGO) build --release -p okkama-cli -p okkama-native -p sparrow-cli -p blade-cli

install: build
	./install.sh --prefix "$(PREFIX)" --no-build

uninstall:
	./install.sh --prefix "$(PREFIX)" --uninstall

test:
	cd $(RS) && $(CARGO) test --release --workspace

dist: runtime
	./tools/package.sh --installer

$(VSIX): vscode-okkama/package.json vscode-okkama/extension.js vscode-okkama/syntaxes/okkama.tmLanguage.json
	cd vscode-okkama && npm install --omit=dev --no-audit --no-fund
	cd vscode-okkama && npx --yes @vscode/vsce@3 package --skip-license --allow-missing-repository -o okkama-vscode.vsix < /dev/null

vscode: $(VSIX)
	@code="$$(command -v code || echo '/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code')"; \
	if [ -x "$$code" ]; then "$$code" --install-extension $(VSIX) --force \
		&& echo "==> reload every open VS Code window: Cmd+Shift+P, Developer: Reload Window"; \
	else echo "VS Code CLI not found: install $(VSIX) via Extensions: Install from VSIX"; fi

clean:
	cd $(RS) && $(CARGO) clean
	rm -f $(STAMP) $(VSIX)
