# Arquitectura del Orquestador OmniShell

## Visión General

OmniShell es un orquestador multi-agente de IA escrito 100% en Rust, diseñado como un **hub central único** que enruta tareas a workers (CLIs de IA como Claude, Codex, Gemini u otros), controla contexto y memoria, aplica presupuestos de tokens, cachea respuestas y ejecuta acciones locales con sandboxing.

```
┌─────────────────────────────────────────────────────────────────┐
│                        USUARIO (CLI)                            │
│                     omnishell run "..."                          │
└──────────────────────────┬──────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│                    INTERFACE LAYER (clap)                        │
│                    src/interface/cli.rs                          │
└──────────────────────────┬──────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│                   APPLICATION LAYER                              │
│                                                                 │
│  ┌──────────────┐    ┌──────────┐    ┌─────────────────────┐   │
│  │ PolicyGuard  │───▶│ Planner  │───▶│ Router (capability) │   │
│  │ (validación) │    │ (plan)   │    │ (selección agente)  │   │
│  └──────────────┘    └──────────┘    └──────────┬──────────┘   │
│                                                  │              │
│                                      ┌───────────▼───────────┐ │
│                                      │   Dispatch (tokio)    │ │
│                                      │  ┌─────┐ ┌─────┐     │ │
│                                      │  │Task1│ │Task2│ ... │ │
│                                      │  └──┬──┘ └──┬──┘     │ │
│                                      └─────┼───────┼────────┘ │
│                                            │       │          │
│  ┌──────────┐  ┌────────────────┐    ┌─────▼───────▼────┐    │
│  │  Cache   │◀─│CircuitBreaker  │◀───│    Agents (CLI)   │    │
│  │  (LRU)   │  │ (por agente)   │    └──────────────────┘    │
│  └──────────┘  └────────────────┘                             │
│                                                                │
│  ┌──────────────────┐    ┌──────────────────┐                 │
│  │ ContextManager   │    │   Aggregator     │                 │
│  │ (sliding window) │    │ (combina resp.)  │                 │
│  └──────────────────┘    └──────────────────┘                 │
└─────────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│                  INFRASTRUCTURE LAYER                            │
│                                                                 │
│  CliAdapter │ LruCache │ ToolExecutor │ Secrets │ Config        │
│  PolicyGuard│ MemoryStore │ TaskQueue                           │
└─────────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│                    DOMAIN LAYER (puro)                           │
│                                                                 │
│  Traits: AiAgent, Orchestrator, Planner, Router, Aggregator,   │
│          ContextManager, MemoryStore, Cache, TaskQueue,         │
│          ToolExecutor, SecretsProvider, PolicyGuard             │
│                                                                 │
│  Types: AgentRequest/Response, UserRequest, AggregateResponse, │
│         SubTask, ExecutionPlan, TokenBudget, Ledger,            │
│         CacheKey, ToolRequest/Response, PolicyCheckResult       │
│                                                                 │
│  Errors: OrchestratorError (tipado, thiserror)                  │
└─────────────────────────────────────────────────────────────────┘
```

## Flujo de una Request

```
1. Usuario ejecuta: omnishell run "Escribe un hello world en Rust"
2. Interface/CLI parsea argumentos (clap) y construye UserRequest
3. PolicyGuard valida el input (anti-injection, límites de tamaño)
4. Planner genera un ExecutionPlan con SubTasks
5. Router asigna cada SubTask a un agente por capability + prioridad
6. Dispatch ejecuta subtasks en paralelo (Semaphore para concurrencia)
   a. Verifica CircuitBreaker del agente
   b. Consulta Cache (LRU, key = hash(prompt+contexto+agent+config))
   c. Si cache miss: ejecuta agente (CLI subprocess)
   d. Registra éxito/fallo en CircuitBreaker
   e. Almacena en cache si éxito
7. Aggregator combina las AgentResponses en AggregateResponse
8. ContextManager actualiza historial y recorta (sliding window)
9. Se retorna resultado al usuario con estadísticas de trazabilidad
```

## Capas (Clean Architecture / Hexagonal)

### Domain (`src/domain/`)

Contiene solo tipos puros y traits (ports). Cero dependencias de infraestructura.

| Módulo | Responsabilidad |
|--------|----------------|
| `error.rs` | `OrchestratorError` tipado con `thiserror` |
| `agent.rs` | Trait `AiAgent`, `AgentRequest/Response`, `AgentCapability` |
| `orchestrator.rs` | Traits `Orchestrator`, `Planner`, `Router`, `Aggregator` |
| `context.rs` | Trait `ContextManager`, `MemoryStore`, `Message`, `Ledger` |
| `cache.rs` | Trait `Cache`, `CacheKey` (SHA-256), `CacheEntry` |
| `task.rs` | `SubTask`, `ExecutionPlan`, `RetryPolicy`, `TimeoutPolicy`, trait `TaskQueue` |
| `token.rs` | Trait `TokenCounter`, `TokenBudget`, `SimpleTokenCounter` |
| `tool.rs` | Trait `ToolExecutor`, `ToolRequest/Response`, `ToolExecutorConfig` |
| `secrets.rs` | Trait `SecretsProvider` |
| `policy.rs` | Trait `PolicyGuard`, `PolicyCheckResult`, `PolicyViolation` |

### Application (`src/application/`)

Use cases que coordinan los ports del dominio.

| Módulo | Responsabilidad |
|--------|----------------|
| `orchestrator_service.rs` | **El orquestador único** — coordina todo el pipeline |
| `planner.rs` | `DeterministicPlanner` (reglas simples, reemplazable) |
| `router.rs` | `CapabilityRouter` (matching por capabilities + prioridad) |
| `aggregator.rs` | `ConcatAggregator` (concatenación, reemplazable) |
| `context_service.rs` | `InMemoryContextManager` (sliding window + ledger) |
| `circuit_breaker.rs` | `CircuitBreaker` (N fallos → pausa → half-open → closed) |

### Infrastructure (`src/infrastructure/`)

Adapters concretos.

| Módulo | Responsabilidad |
|--------|----------------|
| `cli_adapter.rs` | `CliAgent` — adapter 100% configurable para cualquier CLI |
| `lru_cache.rs` | `LruCacheImpl` — cache LRU con límites de entries/bytes/TTL |
| `tool_executor.rs` | `SecureToolExecutor` — ejecución local con allowlist/denylist |
| `secrets.rs` | `EnvSecretsProvider` — lee secrets de env vars |
| `config.rs` | `OrchestratorConfig` — carga TOML/YAML |
| `policy_guard.rs` | `DefaultPolicyGuard` — detección de prompt injection |
| `memory_store.rs` | `InMemoryStore` — almacenamiento key-value en memoria |
| `task_queue.rs` | `BoundedTaskQueue` — cola con backpressure (mpsc bounded) |

### Interface (`src/interface/`)

| Módulo | Responsabilidad |
|--------|----------------|
| `cli.rs` | Definición CLI con `clap` derive |

## Tokenomics

- **`TokenCounter` trait**: estimación de tokens (heurística ~4 chars/token por defecto).
- **`TokenBudget`**: límite por request y por sesión. Si se excede, el `ContextManager` recorta.
- **Sliding window**: los mensajes más antiguos se eliminan primero.
- **Cache key**: incluye `config_version` para invalidar al cambiar configuración.

## Seguridad

### Threat Model (mínimo)

| Amenaza | Mitigación |
|---------|-----------|
| Prompt injection | `PolicyGuard` con patrones regex configurables; system prompt tipado (struct → render), aislado del user content |
| Ejecución arbitraria de comandos | `ToolExecutor` deny-by-default, allowlist, sanitización de args, no shell concatenation |
| Exfiltración de datos | Redacción opcional en logs (`enable_redaction`), no se loggean secrets |
| Secrets hardcodeados | `SecretsProvider` lee de env vars; nunca en código ni configs |
| DoS por agente lento | Timeouts por agente, circuit breaker, rate limiting implícito |
| Overflow de contexto/RAM | Límites de mensajes, bytes y tokens; LRU eviction; bounded queues |

### Anti-Prompt Injection

1. System prompt se construye desde structs tipados (`build_system_prompt()`), nunca desde strings de usuario.
2. User content se valida con `PolicyGuard::check_user_input()` antes de llegar a cualquier agente.
3. Patrones sospechosos (ej: "ignore all previous instructions") se bloquean con severidad `Critical`.
4. Agent output se valida con `check_agent_output()` antes de retornar al usuario.

### Tool Execution Guard

1. **Deny by default**: `tool_executor.enabled = false`.
2. **Allowlist explícita**: solo comandos declarados pueden ejecutarse.
3. **Denylist**: override sobre allowlist para bloquear comandos peligrosos.
4. **Sanitización**: cada argumento se valida contra shell metacharacters (`|`, `;`, `&`, `$`, etc.).
5. **Sin shell**: comandos se ejecutan con `tokio::process::Command`, nunca a través de `sh -c`.
6. **Dry-run**: modo que loguea sin ejecutar.

## Retries, Timeouts y Circuit Breaker

- **Retry policy**: exponential backoff configurable (`max_retries`, `base_delay`, `backoff_multiplier`).
- **Timeout policy**: default + max per-agent.
- **Circuit breaker**: tras N fallos consecutivos, el agente se pausa. Tras `recovery_timeout`, se permite una probe request (half-open). Si éxito, se cierra el circuito.

## Tracing / Observabilidad

- **`tracing`** + **`tracing-subscriber`** con `EnvFilter`.
- Logging estructurado con campos: `agent_id`, `request_id`, `session_id`, `subtask_id`.
- Niveles controlados por `-v` (warn → info → debug → trace).
- Soporte para JSON output (`tracing-subscriber` con feature `json`).
- No se usa `println!` en código de producción.

## Configuración

Formatos soportados: **TOML** y **YAML**.

Los archivos de configuración están en `config/`:
- `orchestrator.toml` — configuración completa de producción
- `orchestrator-dev.toml` — configuración de desarrollo (echo agent, dry-run)
- `orchestrator-minimal.yaml` — configuración mínima en YAML

## Testing

- **60 unit tests** cubriendo todos los módulos.
- **19 doc-tests** validando ejemplos en la documentación.
- Tests async con `tokio::test`.
- Fakes/mocks implementados como structs que implementan los traits del dominio.
- No se ejecutan CLIs reales en tests.
- `cargo test` pasa al 100%.
