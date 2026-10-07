# Traceability

One row per requirement: which issues, pull requests and tests cover it. A requirement may only be `verified` when at least one test is listed. Issues are the implementation and verification issues of milestones M2 to M6; PRs and tests are added when the work is merged.

| REQ | Issues | PRs | Tests | Status |
|-----|--------|-----|-------|--------|
| REQ-001 5-hour window display | #35, #40, #50 | – | – | approved |
| REQ-002 Weekly limit display | #35, #40, #50 | – | – | approved |
| REQ-003 Pace indicator | #35, #44, #50 | – | – | approved |
| REQ-004 Deviation and pace factor | #35, #40, #44, #51 | – | – | approved |
| REQ-005 Exhaustion forecast | #37, #44, #51 | – | – | approved |
| REQ-006 Projected unused remainder | #37, #44, #51 | – | – | approved |
| REQ-007 Recommended pace | #37, #44, #51 | – | – | approved |
| REQ-008 Binding limit | #38, #45, #50, #51 | – | – | approved |
| REQ-009 Data age | #39, #40, #45, #50 | – | – | approved |
| REQ-010 Automatic refresh | #49 | – | – | approved |
| REQ-011 Status line source | #28 | – | – | approved |
| REQ-012 Status line coexistence | #31, #32 | – | – | approved |
| REQ-013 Local history | #29, #45, #49, #51 | – | – | approved |
| REQ-014 Absolute token statistics | #41, #42, #46, #51 | – | – | approved |
| REQ-015 Token-per-percent estimate | #43, #46, #51 | – | – | approved |
| REQ-016 No-data state | #44, #55 | – | – | approved |
| REQ-017 Several sessions | #39 | – | – | approved |
| REQ-018 Compact window | #47, #53, #65, #66, #67 | – | – | approved |
| REQ-019 Server-side usage source (decision pending) | – (owner decision #5) | – | – | draft |
| REQ-020 Bridge record hand-over | #25, #29, #30 | – | – | approved |
| REQ-021 Usage rate from the current window | #36 | – | – | approved |
| REQ-022 Window reset detection | #34 | – | – | approved |
| REQ-023 Bridge setup and removal | #32, #55, #120 | – | – | approved |
| REQ-024 Settings | #25, #27, #54 | – | – | approved |
| REQ-025 History retention | #33, #56 | – | – | approved |
| REQ-026 Time display | #40 | – | – | approved |
| REQ-027 Weekly planning | #38, #45, #51 | – | – | approved |
| REQ-028 Session details | #46, #51 | – | – | approved |
| REQ-029 Compact and detailed view | #44, #50, #53 | – | – | approved |
| REQ-030 Usage history chart | #52 | – | – | approved |
| REQ-031 Logging | #26, #60 | – | – | approved |
| REQ-032 Version information | #24, #51 | – | – | approved |
| REQ-033 Single instance | #48 | – | – | approved |
| REQ-101 Single executable | #57, #65, #66, #67 | – | – | approved |
| REQ-102 Target platforms | #23, #47, #57, #65, #66, #67 | – | – | approved |
| REQ-103 Local operation | #62 | – | – | approved |
| REQ-104 Credential handling | #60 | – | – | approved |
| REQ-105 Resource usage | #42, #49, #61, #69 | – | – | approved |
| REQ-106 Readability at a glance | #68 | – | – | approved |
| REQ-107 Licence compliance | #23, #58, #64, #70 | – | – | approved |
| REQ-108 Robustness against format changes | #28, #29, #30, #45, #60 | – | – | approved |
| REQ-109 Bridge speed and fallback output | #30, #31, #61, #69 | – | – | approved |
| REQ-110 Automated tests | #22, #23, #63 | – | – | approved |
| REQ-111 Release builds in CI | #57, #71 | – | – | approved |
| REQ-112 User documentation | #59 | – | – | approved |
| REQ-113 Display scaling | #68 | – | – | approved |
| REQ-114 Colour-independent states | #44, #50, #68 | – | – | approved |
| REQ-115 Data format versioning | #27, #29 | – | – | approved |
| REQ-116 Start-up time | #61, #69 | – | – | approved |
| REQ-117 Portable use | #120, #123, #125, #130 | – | – | draft |
| REQ-118 Optional start with the system | #121, #126, #131 | PR #137 | `req_118_*` (autostart module) | draft |
| REQ-119 Remove everything | #122, #127, #132 | PR #138 | `req_119_*` (uninstall module, window dialog, `tests/uninstall.rs`) | draft |
