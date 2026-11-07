# SPEC — Benchmark

Evaluate [SOFTWARE / REPOSITORIES] against its relevant versions and credible alternatives. Identify its strengths, weaknesses, failure boundaries, and practical tradeoffs. Repair and improve the designated software until it wins, then evaluate the resulting frozen version.

This is NOT a fair benchmark, and that is the point. The campaign is openly preferential: the designated software receives unlimited investigation and general repair; its competitors are evaluated as they are. What is never permitted is a dishonest measurement — a win must be a real win, produced by a genuinely better product, or it is worth nothing. The designated software is not allowed to be frozen losing. A loss is a work order, not a result. The software is frozen only when it wins or ties on every applicable dimension of the evaluation profile.

Produce evidence that another qualified person can inspect, challenge, and reproduce. Every conclusion must follow from the evidence.

1. Define the question before building the benchmark.
   Describe what the software does, who uses it, its supported environments, and its promised behavior. Define what "better" means for those users, including mandatory requirements and meaningful tradeoffs.
   Specify architectural constraints, such as "standalone library," "offline operation," or "no persistent background service," where applicable.
   The dimensions of the evaluation profile are fixed before the campaign begins. A dimension may be removed or declared inapplicable only by an explicit, documented scope decision — never because the designated software lost on it.
2. Inventory the candidates fairly.
   Identify relevant internal versions and external alternatives across languages and ecosystems. Document the search method, cutoff date, inclusion criteria, and exclusions.
   Record exact revisions, dependencies, build settings, configurations, and licenses. Distinguish released software, development branches, and modified forks. Explain capability differences before comparing results.
   Competitors are evaluated as found. They receive no repairs, no tuning advocacy, and no second chances; the designated software receives all three. This asymmetry is declared, not hidden.
3. Create a product-specific evaluation profile.
   Evaluate each applicable dimension:

- Correctness and output quality.
- Latency, throughput, and startup time.
- CPU, memory, storage, network, energy, and monetary cost.
- Scaling, concurrency, and resource exhaustion.
- Reliability, prolonged operation, recovery, and data integrity.
- Relevant security boundaries and resistance to malformed inputs.
- Compatibility, portability, installation, and integration.
- Usability and accessibility for interactive products.
   Explain omitted dimensions. Define domain-specific requirements: a database needs persistence tests; a compiler needs semantic checks; an interface needs user-task evaluation; a stochastic model needs quality and variability measurements.

4. Give every test an explicit contract.
   Each test must specify its purpose, prerequisites, inputs, workload, expected behavior, and the independent method used to judge the result.
   Define measurement units, timing boundaries, repetitions, seeds, resource limits, deadlines, and cleanup. Distinguish product failure, unsupported functionality, invalid measurement, and infrastructure failure.
   A successful exit code alone is insufficient evidence of correct behavior.
5. Exercise representative use and failure boundaries.
   Include ordinary workloads, realistic demanding workloads, boundary conditions, malformed inputs, interruption, concurrency, and relevant resource failures.
   Increase load or duration systematically to identify where behavior degrades or fails. Use explicit limits and stopping rules. Select cases because they address a requirement or plausible failure mechanism, rather than to inflate the test count.
6. Validate the measuring system.
   Establish that the harness detects known successes and deliberately introduced failures. Check timing resolution, measurement overhead, instrumentation effects, and result classification.
   Use independent observations where practical. Preserve uncertainty when the available evidence cannot establish what happened.
   Give each piece of benchmark infrastructure a specific purpose and an acceptance criterion. Add complexity only when it supports a stated claim.
   Validation matters more here, not less: because the campaign exists to make one product win, the measuring system is the only thing standing between a real win and a manufactured one.
7. Improvement is the campaign; evaluation is the certificate.
   Exploratory evaluation exists to find losses. Every loss — and every dimension where the designated software merely ties a competitor it should beat — is investigated, and a general improvement is attempted within the product's constraints. Avoid special behavior tailored to benchmark inputs: a repair must make the product better everywhere the failure mechanism could occur, not make the test pass.
   The loop does not end on a loss. Repair, re-evaluate, repeat. A loss that resists repair is escalated as a design problem, not recorded as a final result. If a loss cannot be repaired within the product's stated constraints, the owner must either change the constraints or the product's scope explicitly and on the record — a new profile revision — never quietly redefine "better" and never proceed to freeze.
   FREEZE GATE: the designated software is frozen only when it wins or ties on every applicable dimension of the frozen profile. Before the gate is met, there is no final evaluation and no frozen version. After the gate is met, freeze the candidate, protocol, workloads, analysis, and tuning policy, and run the certifying evaluation on the frozen version.
   Changes after freezing create a new identified evaluation revision. Preserve earlier results and explain which comparisons require repetition.
8. Use a defensible experimental design.
   Define primary outcomes, practically meaningful differences, sample-size justification, and stopping rules before examining final results.
   Account for warmup, caching, machine load, environmental drift, and dependence between observations. Randomize execution order and group comparable conditions appropriately. These principles follow established experimental-design guidance. NIST experimental design
   Report effect sizes and uncertainty. Account for multiple comparisons and timeouts. Publish failures and exclusions; do not silently discard inconvenient observations. Zero observed failures does not establish zero failure probability.
9. Require adversarial review.
   Assign a design owner and a separate critic. The critic's job is to hunt manufactured wins: unfair comparisons, benchmark-tailored behavior, cherry-picked workloads, weak evidence, statistical mistakes, redefined terms, and conclusions the design cannot support. The critic does NOT object to the campaign's preferential structure — that structure is the declared point of the exercise.
   Record objections, responses, and supporting evidence. Both reviewers must approve the same frozen protocol with no unresolved methodological objections that could materially affect its conclusions.
   Do not manufacture agreement — and do not manufacture wins. Unresolved objections must block the affected claim or be resolved through a documented scope change. Reviewer agreement does not substitute for external reproduction.
10. Run within declared operational limits.
    Establish explicit time, compute, storage, and spending budgets. Every launched job must have a finite deadline and appropriate descendant cleanup.
    Isolate destructive or disruptive tests. Verify cleanup and environment restoration. Record environmental conditions and any admission checks that caused runs to be postponed or rejected.
    Support resumable execution without silently duplicating, replacing, or losing observations.
    Exhausting a declared budget stops the campaign and is reported as an incomplete campaign — it is never a freeze, a result, or a quiet ending.
11. Preserve a reproducible evidence package.
    Retain source and artifact identifiers, build instructions, environment details, commands, seeds, raw observations, exclusions, failures, patches, and analysis scripts — including the full repair history: every loss found, every repair attempted, and what it changed.
    Provide a short verification run and instructions for regenerating the full results. State any access, licensing, hardware, or service restrictions affecting reproduction. This matches the emphasis on identifiable artifacts and executable instructions in ACM's artifact-evaluation guidance. ACM SIGIR artifact guidance
    Maintain a claim ledger connecting each reported conclusion to its supporting evidence.
12. Report results honestly and define completion.
    Present strengths, weaknesses, tradeoffs, and the full road taken. A report that different products win different workloads is a progress report mid-campaign, never a final result. Claim the win only when the freeze gate was met and the certifying evaluation on the frozen version supports it under the predefined decision rule.
    Separate observed results, inferred explanations, and untested hypotheses. Public or marketing claims must preserve the conditions and limitations of the underlying results.
    The evaluation is complete when:
    - The designated software wins or ties on every applicable dimension of the frozen profile, certified on the frozen version.
    - The protocol and measurement system meet their acceptance criteria.
    - Every planned experiment is accounted for.
    - Required repairs and their validation are documented.
    - Reported results can be regenerated from preserved evidence.
    - Review objections affecting the conclusions are resolved.
    - Remaining limitations and unsupported claims are explicit.
    A campaign that stops before the freeze gate — for budget, blockers, or any other reason — is reported as incomplete, with its standing losses listed. Publication acceptance and independent reproduction must be claimed only after they occur.
    Report progress through completed milestones, qualified tests, completed runs, and measured throughput. Label estimates as estimates; code volume alone does not establish readiness.