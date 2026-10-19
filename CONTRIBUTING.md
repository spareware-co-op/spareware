# Join Spareware with your spare tokens

Anyone can read Spareware. Only invited contributors can push work, and a person reviews every merge, so nobody can damage what others have built.

## How to request to join

1. **Read the [README](README.md)**, especially "Built legally, or not at all".
2. **Open a join request:** [New join request](https://github.com/spareware-co-op/spareware/issues/new?template=join-request.yml). It asks what you'd like to work on, a link to your LinkedIn (or another public profile) so an owner can check your background, and for three promises:
   - you build only with Claude Opus 5.5 at Extra high effort, or a newer, more capable model;
   - you follow the legal rules with no exceptions;
   - any work on an employer's time is cleared with your employer first.
3. **An owner reviews it** and, if it's a yes, invites you to the Spareware Co-op organisation on GitHub. Accept the invite within 7 days or it expires.

## Once you're in

1. Clone this repo and open Claude Code in it.
2. Say "I have spare tokens". Your agent follows [AGENTS.md](AGENTS.md): it shows where every project is up to, offers pieces of work sized to what you have left, and you choose.
3. Your agent checks the piece out with `python tools/piece.py checkout <ID>` (you need the GitHub CLI, `gh`, signed in), works only on that piece's branch, and opens a pull request. Nothing goes straight into `main`, and nobody else can take your piece while you have it.
4. If your tokens run out mid-piece, your agent leaves a handover note on a draft pull request so the next person can carry on.

## What isn't accepted

- Pull requests from people who haven't been invited. They are closed without review.
- Work built with a model below the minimum.
- Anything that can't show where its ideas and code came from.

## Talking with others

Use [Discussions](https://github.com/spareware-co-op/spareware/discussions) for ideas, questions and introductions. Use issues for join requests and work pieces.
