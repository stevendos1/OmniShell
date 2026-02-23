# OmniShell Orchestrator

Orquestador multiagente de IA en Rust, diseñado para enrutar una petición hacia uno o varios agentes CLI (por ejemplo `echo`, `codex`, `claude`, etc.), aplicar validaciones de seguridad y devolver una respuesta agregada.

## ¿Qué hace este programa?

OmniShell:

- recibe una petición por CLI,
- la valida con una capa de políticas (anti prompt-injection),
- selecciona agentes según capacidades,
- ejecuta esos agentes,
- combina sus respuestas,
- y devuelve el resultado en texto o JSON.

Además incluye cache LRU, control de contexto, presupuestos de tokens y configuración por TOML/YAML.

## Requisitos

- Rust **1.75+**.
- Cargo.
- (Opcional) CLIs de agentes que quieras conectar (`codex`, `claude`, `gemini`, etc.).

## Instalación / compilación

```bash
cargo build
```

Compilación optimizada:

```bash
cargo build --release
```

## Ejecución rápida (modo desarrollo)

El repositorio ya trae una configuración de desarrollo que usa `echo` como agente, para probar sin depender de proveedores externos.

```bash
cargo run -- -c config/orchestrator-dev.toml run "Hola OmniShell"
```

También puedes listar agentes activos:

```bash
cargo run -- -c config/orchestrator-dev.toml agents
```

Y revisar health checks:

```bash
cargo run -- -c config/orchestrator-dev.toml health
```

## Uso del CLI

Binario: `omnishell`

```bash
omnishell [OPCIONES] <COMANDO>
```

### Opciones globales

- `-c, --config <RUTA>`: ruta de config (default: `config/orchestrator.toml`).
- `-v, --verbose`: aumenta nivel de logs (`-v`, `-vv`, `-vvv`).
- `--format <text|json>`: formato de salida de `run`.

### Comandos

#### `run`
Ejecuta una petición de usuario.

```bash
omnishell -c config/orchestrator-dev.toml run "Genera un resumen"
```

Opciones de `run`:

- `--session <ID>`: id de sesión (default: `default`).
- `--capability <NOMBRE>`: capacidad preferida (ej. `code-generation`).
- `--max-tokens <N>`: límite de tokens para la petición.

Ejemplo con JSON:

```bash
omnishell -c config/orchestrator-dev.toml --format json run "Analiza este texto"
```

#### `agents`
Lista agentes activos:

```bash
omnishell -c config/orchestrator-dev.toml agents
```

#### `health`
Ejecuta health check de cada agente configurado:

```bash
omnishell -c config/orchestrator-dev.toml health
```

#### `config`
Imprime configuración efectiva cargada:

```bash
omnishell -c config/orchestrator-dev.toml config
```

## Configuración

El programa detecta el formato por extensión y soporta:

- `.toml`
- `.yaml` / `.yml`

Ejemplos incluidos:

- `config/orchestrator.toml`: configuración completa base.
- `config/orchestrator-dev.toml`: configuración de desarrollo (agente `echo`).
- `config/orchestrator-minimal.yaml`: ejemplo mínimo en YAML.

### Estructura principal

```toml
config_version = "v1"
max_concurrency = 8

[[agents]]
id = "echo-agent"
display_name = "Echo Agent"
binary = "echo"
base_args = ["{PROMPT}"]
input_mode = "arg" # stdin | arg | file
prompt_placeholder = "{PROMPT}"
output_format = "text" # text | json | auto
timeout_seconds = 5
max_concurrency = 4
priority = 1
capabilities = ["general", "code-generation"]
enabled = true
env_vars = []
```

### Campos importantes

- `agents[]`: define cada adaptador de agente CLI.
- `cache`: activa cache LRU (`enabled`, `max_entries`, `max_bytes`, `ttl_seconds`).
- `context`: límites de historial (`max_messages`, `max_bytes`, `max_tokens`).
- `token_budget`: límites por request/sesión.
- `retry_policy`: reintentos con backoff exponencial.
- `timeout_policy`: timeout por defecto y máximo.
- `tool_executor`: ejecución local de comandos (recomendado `enabled=false` en producción si no es necesario).
- `policy`: reglas anti-inyección y límites de input.

## Cómo conectar un agente real

1. Crea/edita un bloque `[[agents]]`.
2. Ajusta `binary` al ejecutable instalado en el sistema.
3. Configura `base_args` + `input_mode` según el CLI:
   - `stdin`: envía el prompt por stdin.
   - `arg`: inserta prompt usando `prompt_placeholder`.
4. Define `output_format`:
   - `text`: toma stdout como contenido.
   - `json`: parsea JSON y extrae con `json_content_path`.
   - `auto`: intenta detectar automáticamente.
5. Si necesita API key, usa `env_vars` con valores tipo `"$NOMBRE_ENV"`.

## Logs y observabilidad

Niveles por `-v`:

- sin `-v`: `warn`
- `-v`: `info`
- `-vv`: `debug`
- `-vvv` o más: `trace`

También puedes controlar filtros con `RUST_LOG`.

## Seguridad

- Validación de input/output con `PolicyGuard`.
- Bloqueo de patrones sospechosos (prompt-injection).
- `ToolExecutor` en modo deny-by-default.
- Soporte de redacción (`enable_redaction`) en políticas.

## Pruebas

```bash
cargo test
```

## Solución de problemas

- **`No active agents.`**
  Revisa que al menos un agente tenga `enabled = true` y que su binario exista en `PATH`.

- **Errores de configuración** (`InvalidConfig`).
  Verifica formato TOML/YAML y tipos de campos (durations, listas, etc.).

- **Health FAIL**.
  El ejecutable configurado en `binary` no está instalado o no está en `PATH`.

## Documentación adicional

- Arquitectura detallada: `docs/ARCHITECTURE.md`.
