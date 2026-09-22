CI_BASE ?= origin/develop
CI_FULL ?= 0
CI_PLAN ?= 0
export CI_BASE CI_FULL CI_PLAN

.PHONY: ci ci-full ci-plan
ci:
	CI_PLAN=0 node scripts/ci.mjs

ci-full:
	CI_FULL=1 CI_PLAN=0 node scripts/ci.mjs

ci-plan:
	CI_PLAN=1 node scripts/ci.mjs
