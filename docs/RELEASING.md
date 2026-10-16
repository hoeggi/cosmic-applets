---
name: release
description: Release one or more applets of this workspace - draft a real changelog from the commits since each applet's last tag, propose the next versions, ask for approval, then bump, commit, tag and push so the Release workflow publishes each .deb with its changelog.
argument-hint: <applet>[@version|@major|@minor|@patch] ... | all
disable-model-invocation: true
---

# Releasing applets

This is the release procedure for this repo. You can follow it by hand, or have a coding agent
run it. It is also the `/release` skill for Claude Code (`.claude/skills/release/SKILL.md`
links here), and `AGENTS.md` points other agents to it.

**Input** is `$ARGUMENTS`, or whatever the user asked for. It's a list of applets, each naming
a directory under `applets/`:
- `headsetcontrol`: release one applet with a proposed version
- `headsetcontrol commands`: release several in one go
- `headsetcontrol@0.2.0 commands@patch`: pin an exact version or a bump level
  (`major`, `minor`, `patch`) per applet
- `all`: every applet that has changes since its last tag. Unchanged applets are skipped,
  and the summary says so.

Everything below applies to each selected applet. They are approved together, released in
one commit, and pushed together.

How releases work in this repo:
- Each applet is versioned on its own (`version` in `applets/<applet>/Cargo.toml`) and tagged
  `<applet>-v<version>`.
- Pushing a tag runs `.github/workflows/release.yml` once for that tag. The workflow checks
  the tag against the crate version, builds the `.deb`, and creates the GitHub release. The
  release notes are this version's `## [x.y.z]` section of `applets/<applet>/CHANGELOG.md`.
- So this procedure never creates GitHub releases itself. It only has to get correct
  changelogs, versions, a commit and tags onto `origin`.

Always run git as `git --no-pager …` so nothing waits on a pager. Builds run in the container
through `make`, never with host cargo.

## 1. Preflight (read-only)

Stop and report if any check fails. Don't try to fix it yourself.

- Every named applet has a directory `applets/<applet>/`. If one doesn't, list the
  directories that do and stop.
- The current branch is `main`, and `git status --porcelain` is empty.
- After `git fetch origin --tags`, `main` is neither behind nor diverged from `origin/main`
  (`git rev-list --left-right --count origin/main...main`).
- For each applet, find its previous release:
  `git tag --list '<applet>-v*' --sort=-v:refname | head -n1`. If there is none, this is the
  applet's first release, and the range is the whole history.
- Read each applet's current version from `applets/<applet>/Cargo.toml`. It should equal the
  previous tag's version. Point out any mismatch.

## 2. Collect what changed (per applet)

Look at the commits since the applet's previous tag that touch the applet or the code it
ships with:

```
git --no-pager log --no-merges --format='--- %h %an %ad%n%s%n%n%b' --date=short <prev-tag>..HEAD -- \
  applets/<applet> common Cargo.toml Cargo.lock
```

If an applet has no such commits, there is nothing to release for it:
- If it was named explicitly, say so, and continue with the others.
- If it was selected through `all`, skip it silently. The summary still lists it as
  unchanged.

If no applet is left, stop.

Commit messages are not enough on their own. Read the diffs too
(`git --no-pager show --stat <hash>`, then the relevant hunks), so the changelog describes
actual behaviour. In particular:
- `common/` changes only matter when they change behaviour this applet uses.
- A `Cargo.lock` change matters when it updates libcosmic or another notable dependency. Say
  "Updated libcosmic", not every transitive crate.
- Ignore CI-only, formatting and other purely internal changes, unless they change what the
  user installs (for example packaging or the `.desktop` file).

## 3. Draft the changelog entries (per applet)

Write for someone who installs the applet, not for a code reviewer:
- Use [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) groups, in this order:
  `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`. Only include groups that
  have entries.
- Each bullet is one user-visible change, in plain language, present tense. Don't include
  commit hashes, file names, or internal type names. Mention a config key only when users
  set it.
- Merge related commits into one bullet, and leave out changes that cancel each other.
- A shared change (for example a libcosmic update) goes into the changelog of every applet
  it affects.
- Match the tone and line wrapping (about 100 columns) of the existing `CHANGELOG.md`.

The section header is `## [<version>] - <today as YYYY-MM-DD>`.

## 4. Propose the versions (per applet)

Use the version given for the applet (`@0.2.0`, or `@minor` applied to its current version)
if there is one. Otherwise apply SemVer to its changes:
- Below 1.0.0: breaking changes, such as removed features, config that needs migration, or
  dropped compatibility, bump the **minor** version. New features also bump the minor
  version. Fixes and small changes bump the **patch** version.
- From 1.0.0 on: breaking changes bump major, features bump minor, and fixes bump patch.

Check that no tag `<applet>-v<version>` exists yet, locally or on `origin`.

## 5. Ask for approval (nothing written yet)

Show one summary covering all selected applets:
- for each applet: the current version → proposed version, and its previous tag
- for each applet: the full changelog section, exactly as it will be written
- applets that were skipped as unchanged (with `all`)
- what will happen:
  - the files changed: each `applets/<applet>/Cargo.toml`, each `applets/<applet>/CHANGELOG.md`,
    and `Cargo.lock`
  - the commit message
  - the tags
  - the push command(s)

Then ask the user to pick one of these (as a multiple-choice question if your tool has one):
- **Release**: go ahead exactly as shown
- **Change a version**: ask which applet and which version, then show step 5 again
- **Edit a changelog**: take the user's edits, then show step 5 again
- **Drop an applet**: remove it from this release, then show step 5 again
- **Abort**: stop without changing anything

Don't edit a file, commit, tag or push until the user has chosen **Release** for the exact
versions and text shown.

## 6. Release

1. For each applet, set `version = "<version>"` in `applets/<applet>/Cargo.toml`. Change only
   the `[package]` version line.
2. For each applet, insert its new section into `applets/<applet>/CHANGELOG.md`. Put it
   directly above the previous `## [` section, or at the end of the header if there is none.
3. Run `make check test` once. This refreshes `Cargo.lock` for the new versions inside the
   build container and confirms the tree still builds. If it fails, stop, report the error,
   and leave the edits uncommitted for the user.
4. Check that the only changed files are the expected ones (`git status --porcelain`).
5. Make one commit for the whole release. `Cargo.lock` is shared, so separate per-applet
   commits would need partial staging.
   - Subject:
     - one applet: `<applet>: release v<version>`
     - several: `release: <applet> v<version>, <applet> v<version>`
   - Body: each applet's changelog section under a `<applet> v<version>` line, followed by
     any commit attribution trailer your tool or session requires.
6. Create one annotated tag per applet, all on that commit:
   `git tag -a <applet>-v<version> -m "<applet> v<version>"`.
7. Push the commit and the tags together:
   `git push --atomic origin main <applet>-v<version> …`.
   GitHub starts no workflow runs at all when more than 3 tags are pushed at once. With
   more than 3 applets:
   - push `main` with the first 3 tags in one `--atomic` push
   - push the remaining tags in further batches of at most 3

## 7. Report

Report what was pushed: the commit, and each tag with its version. Also give the links:
- workflow runs, one per tag: `https://github.com/<owner>/<repo>/actions/workflows/release.yml`
- each release, once its workflow finishes:
  `https://github.com/<owner>/<repo>/releases/tag/<applet>-v<version>`

Take `<owner>/<repo>` from `git remote get-url origin`. If a push failed, say so and give the
exact state:
- whether the commit and each tag were created locally
- what has already reached `origin`
- what is left to push
