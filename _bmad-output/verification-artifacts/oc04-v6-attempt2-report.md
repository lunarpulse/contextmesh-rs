# OC-04 v6 (attempt #2) — P3 재판정 결과: INCONCLUSIVE (FINAL)

date: 2026-11-03 · protocol v6.1+r2 · codebook v2.1 · dispatcher_v6.py
verdict: **P3 = INCONCLUSIVE, 최종 확정 (시도 #3 없음 — 계획 명시)**

## 실행 요약

- 주석자: 계열당 1명 × 3 — A6=GLM glm-5.3(z.ai coding API, thinking disabled,
  temp 0) / B6=gpt-6.1-sol(codex CLI) / C6=deepseek-chat(temp 0)
- 우주: 192세션 / 3,979 후보이벤트 (시도 #1과 동일한 annot_A 패킷, 무변경)
- 완료: 3엔진 각 192/192 세션 (전송계층 재시도 3회; deepseek 1세션은
  리터럴 `TYPE: ` basis 접두 기계 정규화로 파싱 — 아래 편차 기록 참조)

## 게이트 통과 내역

- **G-0 completeness+무결성: PASS** — 3엔진 × 3,979행, 세션별 event_index
  완전일치
- **G-1 corpus-wide uncertain ≤15%: 3/3 PASS** — A6 0.48% / B6 1.68% /
  C6 1.01% (시도 #1: 6.5/5.0/8.5% — codebook v2.1 CONSUMPTION-first의
  실질 개선)
- **G-2 세션별 태만: 세션 1개(`e6a1bbedf10b`) 제외** — B6·C6 양쪽 ≥30%
  uncertain. 규칙대로 재발송 1회(원자 교체) 후에도 양쪽 재차 ≥30% →
  세션 전체(64ev) 제외, 축소 universe 3,915ev에서 G-0/G-1 재실행 →
  PASS (A6 0.49% / B6 1.20% / C6 0.49%)

## G-3 신뢰도 게이트: FAIL (시도 #2 종결 사유)

| pair | binary κ | (기준 ≥0.60) | 5-label κ | (기준 ≥0.50) | 판정 |
|---|---|---|---|---|---|
| A6–B6 (GLM↔GPT) | 0.5362 | ✗ | 0.5054 | ✓ | FAIL |
| A6–C6 (GLM↔DS)  | 0.6054 | ✓ | 0.5648 | ✓ | PASS |
| B6–C6 (GPT↔DS)  | 0.3895 | ✗ | 0.3859 | ✗ | FAIL |

- 3쌍 전부 통과 필요 → **G-3 FAIL** → 계획에 따라 시도 #2 FAIL,
  G-5/G-4 미실시(원자적 순서: G-5는 G-3 PASS 시에만)
- 참고(비게이팅): 만장일치 비-uncertain 채택 2,525 / all-uncertain 17 /
  분할 1,373 (of 3,915)

## 시도 #1 대비 해석

- 쌍둥이 블록투표 제거(A″ 폐지, 계열당 1명)에도 **교차계열 κ는 여전히
  게이트 미달** — A6↔B6는 0.419/0.475→0.536/0.505로 유의미 향상
  (codebook v2.1 효과)이나 binary 0.60 미달, B6↔C6는 0.39로 최악
- 결론: 태만·블록투표·경계 모호성을 각각 제거한 뒤에도 **서로 다른
  모델 계열 간 라벨 기준 차이는 구조적** — 시도 #1 판정(INCONCLUSIVE)은
  데이터 결함이 아니라 실제 현상임이 2차 확인됨
- post-mortem 3원인 가설 중 D1/D2는 이번 시도로 대부분 해소(uncertain
  0.5~1.7%, 태만 세션 1개뿐) — 잔존 원인은 계열 간 판단 기준 차이 자체

## 기록된 편차 (fail-closed 문서화)

1. GLM 전송계층: glm-5.3 reasoning 모델의 `reasoning_content` 소진으로
   빈 content → `thinking: disabled` + max_tokens 12,000 적용 (프롬프트·
   코드북 불변; 스모크 재시도 1회 규칙 내 통과)
2. dispatcher deepseek 인라인 페이로드 SyntaxError(`}).\n encode()`)로
   1차 디스패치 C6 0/192 — 스모크 검증 ds_call.py 파일 호출로 교체 후
   전량 재실행
3. C6 세션 `8cfd928e3256`: deepseek가 basis에 리터럴 `TYPE: ` 접두를
   결정론적으로 삽입 → 접두만 기계 제거하는 basis 정규화로 파싱
   (라벨·의미 불변, 5-15단어 재검사). f93d9e4bbbea는 순수 재시도로 통과
4. G-2 제외 세션 1개(64ev) — 계획에 명시된 제외 경로, G-4 분모 규칙과
   무관하게 시도 #2는 G-3에서 종결되어 미반영

## 최종 상태

- **P3 = INCONCLUSIVE (최종, 소급 불변)** — 시도 #1 `d554a2c` 판정과
  동일 결론, 독립 2차 확인
- 코퍼스 표기: "human-audited cross-family consensus gold" 구성 불가
  확인 (2회 독립 시도)
- 다음 단계 결정 필요: P4/P5 트랙 방향 (창업자)
