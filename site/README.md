# The site

The Site workflow (`.github/workflows/site.yml`) publishes this folder to GitHub Pages on every push to main, with each program's demo and feature map, and two pages it builds:

- `contributors.html`, the contributor map, made by `tools/contributor-map/build.py` from the merged pull requests.
- `kernel-progress.html`, where the geometry kernel stands, made by `tools/kernel-progress/build.py`: it reads the board (chapter 30), the decision log (chapter 32), MAP.json and main's history for what is on main, and the merge trains, open pull requests and piece branches on GitHub (read-only, with the workflow's token) for what is built and waiting to land. `python tools/kernel-progress/test_build.py` tests its counting; run `python tools/kernel-progress/build.py out.html --offline` from the repository root to build it without GitHub.
