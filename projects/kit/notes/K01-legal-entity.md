# K01: Legal structure for Spareware Co-op

*Decision note. Researched 2026-10-18. Not legal advice: a lawyer must check the constitution and every item marked* unverified.

## The question

What legal form should Spareware take so that it is owned by its members, can run at-cost platforms in New Zealand and other countries, keeps the code and the name safe for everyone, and never pays surplus to outside investors?

## Recommendation

**A federation of two kinds of entity**, starting in New Zealand:

1. **A steward that holds the code and the name.** An incorporated society (Incorporated Societies Act 2022) owns the Spareware trademarks and logo, stewards the open-source code, and licenses the name to member co-ops. It can never distribute surplus to members, which is exactly the lock the code and brand need. It could later seek charity registration for donations, if its purposes qualify.
2. **Operating co-ops that run the platforms.** The first is a New Zealand co-operative company (Co-operative Companies Act 1996), working name *Spareware Co-operative Limited*. Workers and users who trade with it are its members: one member, one $1 share, one vote, and no outside investor shares. Surplus goes back as rebates or lower fees.
3. **Going international.** Each new country gets its own operating co-op (or a branch, where local law allows), with its own local licences, members and votes. It joins the federation under the steward's name licence and shares the same code and kit. This is the model CoopCycle uses: an association holds the software, and local co-ops run the service.

## Why

| Need | How the recommendation meets it |
| --- | --- |
| Members own it | Co-op company: at least 60% of votes must sit with transacting shareholders; the constitution makes it one member, one vote |
| Surplus never goes to investors | No investor share class is ever issued; changing that needs a special resolution |
| "Co-op" in the name | Only a registered co-operative company may use it |
| Code and name stay safe | The steward society can't distribute surplus or sell out; it licenses the name only to co-ops that keep the rules |
| International | Local co-ops meet local law (licences, tax, employment) while sharing one codebase and brand |
| Licences per country | Each operating co-op holds its own licences, such as NZ's small passenger service licence for Spare Ride |

## The options compared

| Structure | Who owns and votes | Can pay workers | Surplus to members | Tax | Fits Spareware |
| --- | --- | --- | --- | --- | --- |
| Co-operative company (Co-operative Companies Act 1996) | Transacting shareholders hold at least 60% of votes; one vote each by constitution | Yes | Yes, as rebates or dividends | 28%; co-ops can elect to deduct some distributions, but the election covers only NZ-resident members and payouts tied to trading stock, so service rebates are probably taxed first (*unverified*) | **Operating co-ops** |
| Incorporated society (Incorporated Societies Act 2022) | Members, no shares | Yes, at arm's length | No: surplus to members counts as "financial gain" | 28% unless exempt | **Steward of the code and name** |
| Charity (trust or company, Charities Act 2005) | Trustees or board | At market rate | No | Exempt; donee status | Steward only, if its purposes qualify; never the platforms |
| Ordinary company with a co-op constitution (Companies Act 1993) | Any shareholders | Yes | Yes | 28% | Weaker lock, and can't use "co-op" in its name |

## Evidence

- **Co-operative companies:** the principal activity must be a "co-operative activity" stated in the constitution, and transacting shareholders must hold at least 60% of voting rights. A transacting shareholder supplies goods or services to the co-op, buys them from it, or has other commercial dealings with it ([Companies Office](https://companies-register.companiesoffice.govt.nz/help-centre/before-you-start-a-company/choosing-a-type-of-company-for-your-business/), [Anderson Lloyd](https://www.al.nz/co-operative-companies/), [s 4](https://www.legislation.govt.nz/act/public/1996/0024/latest/DLM376828.html)). Both workers (who supply services) and users (who buy them) appear to qualify, but no NZ case on a mixed worker and user co-op was found (*unverified*).
- **Tax on co-op distributions:** [IRD 2010](https://www.taxtechnical.ird.govt.nz/new-legislation/act-articles/taxation-annual-rates-trans-tasman-savings-portability-kiwisaver-and-remedial-matters-act-2010/distributions-to-co-operative-company-members), [IRD 2006](https://www.taxtechnical.ird.govt.nz/new-legislation/act-articles/taxation-depreciation-payment-dates-alignment-fbt-and-miscellaneous-provisions-act-2006-2006-no-3-/treatment-of-distributions-from-cooperatives). Pricing at cost keeps surplus small, so the tax point matters less.
- **Incorporated societies:** passing surplus or share capital to members is "financial gain" and not allowed; wages, payment for services and arm's-length deals are ([Registrar guidance](https://is-register.companiesoffice.govt.nz/help-centre/running-your-incorporated-society/restrictions-on-money-making-activities)). Needs at least 10 members, a committee of at least 3 and a disputes procedure ([Registrar](https://is-register.companiesoffice.govt.nz/help-centre/starting-an-incorporated-society/applying-to-incorporate-a-society/)).
- **Charities:** private benefit must be incidental ([Charities Services](https://www.charities.govt.nz/ready-to-register/need-to-know-to-register/charitable-purpose/public-benefit-and-charitable-purpose)). The NZ Open Source Society was a registered charity (CC42367), a precedent for a code steward ([register](https://register.charities.govt.nz/Charity/CC42367)).
- **NZ precedents:** Loomio Cooperative Limited is a registered co-operative company of worker-members (one $1 share, one vote) that owns Loomio Limited ([register](https://app.companiesoffice.govt.nz/companies/app/ui/pages/companies/7630017/detail), [constitution](https://www.loomio.coop/constitution.html)). Protozoa Limited and Truestock Limited are also registered co-operative companies ([Protozoa constitution](https://handbook.protozoa.nz/constitution.html), [Truestock](https://app.companiesoffice.govt.nz/companies/app/ui/pages/companies/8179509/detail)). Enspiral Foundation Ltd is an ordinary company with a charitable-style constitution ([handbook](https://handbook.enspiral.com/foundation/)).
- **International model:** CoopCycle's association holds the software while local courier co-ops run the service ([CoopCycle federation](https://coopcycle.org/en/federation/)).

## Practical steps

1. A lawyer drafts both constitutions. Starting points: Cooperative Business NZ's starter kit ([nz.coop](https://nz.coop/member-benefits/co-op-constitution-starter-kit)) and Loomio's public constitution (check its licence before reusing text).
2. Register the steward society (NZ$88.89 + GST) and the co-operative company (about NZ$130-160 incl. GST with name reservation) ([society fees](https://is-register.companiesoffice.govt.nz/help-centre/forms-and-fees/fees/), [company fees](https://companies-register.companiesoffice.govt.nz/help-centre/managing-your-online-account/schedule-of-fees/)).
3. At least one director of the co-op must live in NZ (or in Australia and be a director of an Australian company) ([Companies Office](https://companies-register.companiesoffice.govt.nz/help-centre/company-directors/who-can-be-a-director/)).
4. Check the 2022 Small Co-operatives exemption still covers offering $1 member shares without a full disclosure document ([exemption](https://www.legislation.govt.nz/secondary-legislation/pco-drafted/2022/325/en/latest/)).
5. Later, per program: Financial Service Providers Register if needed (NZ$300 + NZ$600 levy + GST) ([FSPR](https://fsp-register.companiesoffice.govt.nz/help-centre/registering-a-financial-service-provider-fsp/as-an-entity/)), the small passenger service licence for Spare Ride, and anti-money-laundering compliance for the marketplaces.
6. Assign the Spareware name and logo to the steward, and set how contributors' code is licensed to it (with the licence decision, piece A14).

## Open risks

- A mixed worker and user co-op has no NZ test case; a lawyer should confirm members on both sides count as transacting shareholders.
- Co-op tax deductions may not cover service rebates; at-cost pricing reduces the impact.
- Overseas members: no restriction found in the Act (*unverified*), but the tax election only helps NZ-resident members.
- Work done on employers' time needs each contributor's employer to agree before it can be given to the steward.
- Each country's operating co-op needs its own legal advice.

## Status

In review: an owner accepts or changes this recommendation, then a lawyer is engaged.
