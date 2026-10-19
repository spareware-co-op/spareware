# RES01: Open or anonymous peer review

## The question

Spare Research is free to read, free to publish, international and at cost, and it has an AI-assisted section. Which peer review model should it use? We compare five models on review quality, fairness and bias, reviewer willingness and safety (especially early-career and minority reviewers), speed, and fit. One systematic review found 122 definitions of "open peer review", so open identities and open reports are treated as separate choices.

Marks: "(checked)" means the source was read; "(summary only)" means only an abstract or secondary summary was seen; "[unverified]" means it could not be confirmed.

## Options

| Model | Review quality | Fairness and bias | Willingness and safety | Speed | Fit for Spare Research |
| --- | --- | --- | --- | --- | --- |
| Single-anonymous (reviewers see authors, not the reverse) | No edge shown over the others | Favouring of famous authors seen in some studies; evidence mixed | Safe for reviewers; reports stay hidden | Baseline | Cheap, but hidden reports cannot be checked for poor or AI-written reviews |
| Double-anonymous (both sides hidden) | Blinding had no measured effect on quality | Can cut prestige bias; masking can leak; opt-in use is self-selected | Safe for reviewers | Extra masking work | Hard with public preprints; reports still hidden unless also published |
| Open identities (signed, names shown) | Small or no gain | Exposes conflicts, but juniors may soften or decline | More declines (35% versus 23%); most surveyed oppose; reprisal risk | Mixed | Poor as a rule; fine as a choice |
| Open reports (reports published, reviewer may stay anonymous) | No harm found; small possible gain | Public scrutiny; no direct proof it cuts bias | Willingness held after adjusting for a time trend in the one large pilot | No significant delay found | Strong fit |
| Mixed or opt-in (reports by default, optional signature, optional author-anonymous route) | As open reports | Signers skew positive; needs care | Best balance if staying anonymous is easy | As open reports | Strong fit; more to build |

## Evidence

- **Terms.** 122 definitions reduce to 22 setups built from seven traits. [Ross-Hellauer 2017](https://pmc.ncbi.nlm.nih.gov/articles/PMC5437951/) (checked).
- **Signed versus anonymous trial.** Quality 3.09 (signed) versus 3.06 (anonymous); time 2.20 versus 2.25 hours; 35% of reviewers asked to be identified declined, versus 23%. [van Rooyen 1999](https://pmc.ncbi.nlm.nih.gov/articles/PMC27670) (checked).
- **Other signing trials.** Signed reviews rated higher and more courteous but slower: [Walsh 2000](https://www.cambridge.org/core/journals/the-british-journal-of-psychiatry/article/open-peer-review-a-randomised-controlled-trial/1F81447FC67B3BAFDCCCCE82B6C7A187) (summary only). Posting signed reviews online: no important quality effect, possibly fewer willing reviewers: [van Rooyen 2010](https://www.bmj.com/content/341/bmj.c5729) (summary only).
- **Meta-analysis of 22 trials.** Revealing reviewer identity raised quality slightly (SMD 0.14); time unchanged; blinding changed neither quality nor rejection. [Bruce 2016](https://doi.org/10.1186/s12916-016-0631-5) (abstract checked).
- **Open reports pilot.** 18,525 reviews at five journals: after adjusting for time trends, publishing reports did not significantly change willingness, recommendations or turnaround; only 8.1% agreed to be named. Not randomised. [Bravo 2019](https://www.nature.com/articles/s41467-018-08250-2) (checked).
- **OpenReview and ICLR.** Public reviews, anonymous reviewers ([data study](https://arxiv.org/abs/2103.05885), summary only). Fully open ICLR reviews were slightly better on correctness and completeness than partly open NeurIPS reviews; small, not causal. [Rao 2025](https://arxiv.org/abs/2511.23439) (abstract checked).
- **Prestige bias.** 87% recommended acceptance with author names visible versus 68% masked: [Okike 2016](https://jamanetwork.com/journals/jama/article-abstract/2556112) (summary only). Single-anonymous reviewers favoured famous authors: [Tomkins 2017](https://pmc.ncbi.nlm.nih.gov/articles/PMC5715744/), challenged by [Stelmakh 2019](https://arxiv.org/abs/1912.13188) (both summary only).
- **Opt-in double-anonymous.** 12% of 128,454 Nature-journal manuscripts chose it; choosers were more often from less prestigious institutions and were rejected more often. [McGillivray 2018](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC6097313/) (summary only).
- **Gender.** No sign that review penalised women's manuscripts across 145 journals. [Squazzoni 2021](https://doi.org/10.1126/sciadv.abd0299) (summary only). No study found on open identities and minority reviewers: a gap.
- **Surveys.** Of 3,062 researchers, 50.8% said open identities would make review worse, 65.2% said signing would soften criticism, 65.4% found open reports useful. [Ross-Hellauer 2017 survey](https://journals.plos.org/plosone/article?id=10.1371/journal.pone.0189311) (checked). In machine learning, over 80% back releasing reviews of accepted papers, 27.1% for rejected ones. [Rao 2025](https://arxiv.org/abs/2511.23439).
- **Signing rates.** eLife: 20% shared names, 33% with more encouraging wording ([2016](https://elifesciences.org/inside-elife/e3e90410/increasingtransparency-in-elife-s-review-process)); early-career reviewers disclosed less ([2019](https://elifesciences.org/inside-elife/958c61d1/early-career-reviewers-reflections-on-focused-inclusion-in-reviews-at-elife)). PeerJ: about 40% signed in 2017 ([Clarivate](https://clarivate.com/academia-government/blog/whos-using-open-peer-review/)). All checked.
- **Harm from a leak.** In November 2025 a software flaw exposed author and reviewer identities for many ICLR 2026 submissions; collusion attempts, threats and harassment followed. [ICLR retrospective](https://blog.iclr.cc/2026/03/31/a-retrospective-on-the) (checked).
- **Journal practice.** eLife publishes public reviews with each Reviewed Preprint ([process](https://elifesciences.org/about/peer-review)); whether reviewers are named now is [unverified]. Open Research Europe names reviewers ([LIBER FAQ](https://libereurope.eu/open-research-europe-ore-faqs/)). PeerJ publishes review histories, signing optional ([PeerJ](https://peerj.com/benefits/review-history-and-peer-review), summary only). Nature publishes reports for every new paper since June 2025, reviewers anonymous unless they choose ([Springer Nature](https://group.springernature.com/gp/group/media/press-releases/transparent-peer-review-now-standard-for-nature/27788498)). PLOS lets authors opt in ([PLOS](https://explore.plos.org/reviewer-resources/open-peer-review)).
- **AI rules.** Nature lets reviewers use AI to support review but not upload manuscripts to public tools, delegate judgement, or skip disclosure ([Nature](https://www.nature.com/nature-portfolio/editorial-policies/ai)). ICLR 2026 required disclosure and found hallucinated references widespread ([retrospective](https://blog.iclr.cc/2026/03/31/a-retrospective-on-the)). Optional LLM feedback led 26.6% of ICLR 2025 reviewers to update reviews, with no change in acceptance ([ICLR blog](https://blog.iclr.cc/2025/04/15/leveraging-llm-feedback-to-enhance-review-quality/)).

## Recommendation

Open reports with optional signatures; editors always know identities.

1. Publish each review report and author response with every published item, with its own permanent link. Reports on declined submissions stay private unless the author opts in.
2. Reviewers choose whether to sign; the default is anonymous. Ask once, after the report is filed, in neutral words. Never publish signing rates by career stage.
3. Editors always know who reviewed. Keep identities in a locked store with logged access; no public search or API may link reviewers to papers.
4. Reviewers declare conflicts of interest before accepting; the declaration is published with the report.
5. Reviewers declare AI use (tool and task) on the report form, published. No pasting unpublished manuscripts into public AI tools; the judgement must be their own. Optional private AI feedback may be offered.
6. The AI-assisted section follows the same rules; reviewers check cited references by hand.
7. Reviewers see author names at launch. Offer an author-anonymous route later as an opt-in, once we have our own data (our judgement, not a finding).
8. Review the policy after 12 months: invitation acceptance, time to first report, private sign rate by career stage, report quality.

## Open risks

- The evidence is thin: mostly biomedicine, a few small trials, one non-randomised pilot, nothing on minority reviewers.
- Candour may drop; watch tone.
- Identity data is a target; security and a breach plan cost money on an at-cost budget.
- Wording can pressure early-career reviewers into signing.
- AI declarations are self-reported; public reports may be scraped for model training.
- Publishing critical reports across countries may carry defamation or data-protection risk: needs legal review.
- Moderation workload for abusive or empty reports is not estimated.
- Source gaps: F1000Research pages not opened; some pages blocked; several studies read as summaries only.

## Sources

Every source is linked in Evidence above. Researched 20 October 2026.
