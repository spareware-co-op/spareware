# RES05: Long-term preservation

## The question

If Spare Research closes, how do the papers, their code and their data stay readable for decades? Which archives fit a small not-for-profit that publishes free to read, worldwide, at cost? Prices are US dollars from each archive's own page, read on 20 October 2026. "Unverified" means it could not be confirmed on an official page.

## Options

| Archive | Run by | Cost (official page) | What it needs from us | Terms that matter |
| --- | --- | --- | --- | --- |
| CLOCKSS | US not-for-profit; board of librarians and publishers; 12 university nodes | 2027 table: $285 a year under $250k revenue, up to $33,370. Ingest $0.25 per article (first 100 free), $3.00 per book (first 10 free). No free tier | Sign the Participating Publisher Agreement; let it harvest, or send files | Dark until the board triggers release, then a Creative Commons licence, open to everyone |
| Portico | ITHAKA, a nonprofit | Journals: $250 a year under $250k revenue, up to $81,960. Open access revenue counts. Books have their own fees | An archive licence, content delivered, a yearly contribution (partly unverified) | Dark. Access after a trigger is for participating libraries, but open access titles can stay open to all |
| LOCKSS software | LOCKSS Program, started by Stanford Libraries | Software is free. Each node host pays its own hardware and staff (network cost unverified) | We recruit and run several independent nodes | 3-Clause BSD licence |
| PKP PN (a LOCKSS network) | PKP; nine nodes at universities and a national centre | Free | OJS 3.1.2 or newer, an ISSN, the plugin, its Terms of Use | Dark; OJS only |
| Internet Archive Scholar and Fatcat | Internet Archive, a US non-profit library | No cost, no guarantee | Keep pages crawlable: plain HTML, direct PDF links, no login wall | Public copies in the Wayback Machine. Fatcat server code is AGPLv3 (learn from only) |
| Software Heritage | Inria initiative with UNESCO | No price found (unverified) | A public repository address | Archives all public code; each project keeps its own licence |
| Zenodo | CERN, with OpenAIRE | Free upload | A free account, the right to share, a licence on every public file; 50 GB per record | Kept for the life of CERN; best efforts to move content if it closes |
| National legal deposit | National libraries, under national law | Usually no fee (unverified) | Deposit copies, or the library harvests; rules differ by country | Often read on library premises only |

- CLOCKSS and Portico are the two proven dark archives. Both charge a small yearly fee at the lowest tier.
- A LOCKSS-style network costs only what the node hosts spend, but it needs partners who stay for decades.
- The Internet Archive costs nothing and promises nothing: a bonus copy.
- Software Heritage and Zenodo cover code and data. Zenodo also passes public software to Software Heritage.

## Evidence

- The risk is real: 174 open access journals vanished from the web between 2000 and 2019 ([arXiv 2008.11933](https://arxiv.org/abs/2008.11933)).
- CLOCKSS fees, nodes and trigger release: [join page](https://clockss.org/?p=2338), [how CLOCKSS works](https://clockss.org/about/how-clockss-works/), [triggered content process](https://documents.clockss.org/index.php/CLOCKSS:_Extracting_Triggered_Content).
- Portico fees, terms and audit: [fees](https://www.portico.org/?p=2), [terms](https://www.portico.org/terms/), [participants](https://www.portico.org/for-participants/), [why Portico](https://www.portico.org/why-portico/).
- LOCKSS licence and aims: [licence](https://www.lockss.org/license), [about](https://www.lockss.org/about/).
- PKP PN: [preservation](https://pkp.sfu.ca/pkp-preservation/), [PKP PN](https://pkp.sfu.ca/pkp-pn/), [docs](https://docs.pkp.sfu.ca/pkp-pn/en/).
- Internet Archive Scholar and Fatcat: [about](https://scholar.archive.org/about), [blog](https://blog.archive.org/?p=22580), [Fatcat README](https://github.com/internetarchive/fatcat).
- Software Heritage and Zenodo: [Save Code Now paper](https://ar5iv.labs.arxiv.org/html/1909.10760), [Zenodo to Software Heritage](https://zenodo.org/communities/software-heritage/curation-policy), [Zenodo policies](https://about.zenodo.org/policies/), [Zenodo terms](https://about.zenodo.org/terms/).
- Legal deposit: [UNESCO guidelines](https://unesdoc.unesco.org/ark:/48223/pf0000121413), [IFLA statement](https://ifla.org/?p=170672), [UK non-print guidance](https://www.gov.uk/government/publications/guidance-on-the-legal-deposit-libraries-non-print-works-regulations-2013), [New Zealand notice](https://www.legislation.govt.nz/secondary-legislation/pco-drafted/2006/118/en/latest/).
- Unverified: the 2026 CLOCKSS agreement terms (the PDF did not load), Software Heritage's pricing and terms, and whether a zero-revenue platform gets the lowest fee tier.

## Recommendation

1. Start with a free, self-run LOCKSS-style network, copying the PKP PN design, on the BSD-licensed LOCKSS software. Recruit three to five partner libraries or not-for-profits on different continents. Keep it dark until a trigger.
2. Publish every item with a stable address, plain HTML and PDF copies and an open licence, and never block crawlers, so archives (including the Internet Archive, for free) can copy and release it.
3. Add a dark archive when money allows: CLOCKSS first (lowest yearly fee at our size, release to everyone by default), Portico second.
4. Send platform code and research code to Software Heritage, and data sets to Zenodo.
5. Handle legal deposit per country pack: record each country's duty, the receiving library and how to deposit.
6. Name a successor body and write the trigger steps down now.

## Open risks

- Fee tiers use publishing revenue; a free platform may qualify for the lowest tier, but this is unconfirmed. Ask CLOCKSS and Portico.
- A self-run network is only as strong as its partners; someone must run trigger release.
- PKP PN is for OJS only; without OJS we run our own network.
- Software Heritage's terms are unconfirmed, and it is in a funding transition.
- Zenodo lasts as long as CERN backs it.
- Legal deposit differs by country and may not cover an international online platform.
- Fee tables change every year.

## Sources

Every source is linked in Evidence above. Researched 20 October 2026.
