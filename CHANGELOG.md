# Changelog

All notable changes to this project will be documented in this file.

This project follows semantic versioning.

## [Unreleased]

### Changed

- **Breaking:** `AskRequest.dataset_id: Option<Value>` is replaced by
  `AskRequest.dataset_ids: Option<Vec<String>>`, and `AskOptions.dataset_id` likewise becomes
  `AskOptions.dataset_ids`. `POST /intelligence/query` retired the singular `dataset_id` and answers
  any request carrying it with a 400 naming `dataset_ids` as the replacement.
- `AskOptions::with_dataset` now *adds* a dataset to the scope, so repeating it widens the scope.
- `AskOptions::with_all_datasets` clears the scope instead of sending the retired `"all"` sentinel —
  an absent `dataset_ids` is how the API says "every dataset the caller can see".
- `DatasetService::ask_with` / `ask_stream` scope to the dataset's own id via `dataset_ids`.
- Every `AskOptions` field, `dataset_ids` included, stays optional and defaults to `None`; an empty
  or `"all"`-only scope is omitted from the request rather than sent as `[]`.

### Added

- `AskOptions::with_datasets(ids)` for scoping one question to several datasets.

## [0.4.0] - 2026-08-20

### Added

- Add `GitHubSource` and `GitLabSource` typed source builders.
- Add `client.sources().create_github(...)` and `client.sources().create_gitlab(...)`.

## [0.3.0] - 2026-07-20

### Added

- Add typed metadata-schema fields when creating datasets.
- Add metadata-schema merge/patch and full replacement operations.
- Document create, merge, and replace schema workflows.

## [0.2.0] - 2026-07-14

### Added

- Add organization OpenAI API key secret helpers.
- Add dataset creation helper that stores an OpenAI API key and references the stored secret.
- Add vector deletion helpers with optional write concern.

## [0.1.0] - 2026-07-02

### Added

- Initial public-ready package baseline for VectorAmp SDK/CLI migration to GitHub.
- GitHub Actions CI workflow.
