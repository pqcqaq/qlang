# Phase 6 LSP 与编辑器语义收口

## 目标

Phase 6 的任务不是一次做完 project-wide IDE，而是在已有 same-file 语义边界上，把 query、rename、completion、semantic tokens 和 LSP bridge 系统性收口成一套稳定 editor semantics。

核心交付：

- same-file references / rename
- completion surface parity
- semantic-token parity
- diagnostics bridge parity
- direct/member/type-namespace/callable/value/variant truth surface aggregate
- analysis / LSP 一致性回归

## 合并后的切片结论

### 1. Query truth surface

这一大组切片把 query surface 的根边界做稳：

- direct symbol / member query precision
- explicit field-label query precision
- variant query precision
- callable surface aggregate
- type-namespace surface aggregate
- global value item surface

核心原则始终一致：

- `QueryIndex` 是唯一 editor-facing truth source
- LSP bridge 只做协议映射，不复制语义遍历

### 2. Rename surface

这一组切片把 same-file rename 从脚手架推进到可用边界：

- function / const / static / struct / enum / trait / type alias / import / field / unique method / local / parameter / generic / variant
- shorthand binding rename
- shorthand field rewrite
- item shorthand rename coverage
- deeper struct-like shorthand tokens keep lexical rename fallback without field-label expansion
- lexical rename parity
- type-namespace rename coverage
- root value item rename parity
- deeper variant-like member chains stay deliberately closed for rename

### 3. Completion surface

这一组切片把 completion 扩成多类 surface，同时补齐 editor-facing parity：

- lexical value completion
- type-context completion
- stable member completion
- parsed variant-path completion
- import alias variant / struct-variant follow-through
- deeper variant-like member chains stay deliberately closed
- deeper struct-literal / pattern variant-like paths stay deliberately closed
- deeper struct-like field-label paths stay deliberately closed
- deeper struct-like shorthand tokens keep lexical fallback
- escaped identifier completion 保真
- candidate-list parity
- filtering parity

### 4. Semantic-token surface

这一组切片把 occurrence-based semantic tokens 做成稳定 editor 契约：

- direct semantic-token parity
- direct member semantic-token parity
- callable semantic-token parity
- type-namespace semantic-token parity
- import alias semantic-token parity
- lexical semantic-symbol parity
- deeper variant-like member chains stay deliberately closed for semantic tokens
- deeper struct-literal / pattern variant-like paths stay deliberately closed for rename / semantic tokens
- deeper struct-like field-label paths stay deliberately closed for query / rename / semantic tokens
- deeper struct-like shorthand tokens still project lexical local / binding / import identity across references / semantic tokens as well

### 5. Aggregate hardening

大量后续切片的真正价值不是“再加一个功能”，而是把已经有的 same-file truth surface 做成不会漂移的 editor 契约：

- callable surface aggregate refresh
- direct symbol/member aggregate refresh
- value candidate-list refresh
- type-context candidate-list refresh
- variant candidate-list refresh
- impl-preferred filtering refresh
- lexical visibility filtering refresh
- diagnostics severity / label fallback bridge parity
- UTF-16 / CRLF position-range bridge parity
- semantic-token UTF-16 / CRLF column parity

## 当前架构收益

P6 现在已经把 same-file editor semantics 的主干收敛成一条稳定路径：

- analysis 与 LSP 共用同一份 symbol identity
- hover / definition / references / rename / completion / semantic tokens 不再各自为战
- import alias、field、variant、method、type namespace 这些最易漂移的 surface 已进入系统性回归

## 当前仍刻意保留的边界

- cross-file / workspace index
- parse-error tolerant dot-trigger completion
- deeper module-path completion / navigation
- ambiguous member completion
- ambiguous method rename
- code actions / inlay hints / call hierarchy
- project-wide rename

## 归档

本阶段原始切片稿已归档到 [`/plans/archive/phase-6`](/plans/archive/index)。
