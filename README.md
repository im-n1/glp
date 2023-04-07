# GLP

<p align="center">
    <img src="https://gitlab.com/imn1/glp/-/raw/master/assets/screen.png">
</p>

Small CLI tool for fetching Gitlab pipeline states and other info.

## How to use
App uses following data sources:

- environment variable `GLP_PRIVATE_TOKEN` - your Gitlab
  personal API token
- positional argument "project ID" - Gitlab project ID
  pipelines should be fetched for **or** a `.glp` file
  with project ID (which makes it the best candidate for
  your global `.gitignore` file when you put the file into
  your every project)

## Example usage
```
$ GLP_PRIVATE_TOKEN=123 glp 456  # fetches pipelines for project with ID 456
```

## How to install

1. clone this repository
2. run `cargo install --path=.`

or download prebuild binaries for `amd64`
[here](https://gitlab.com/imn1/glp/-/packages/).

## Changelog

### 0.1.2
- space between pipelines added
- added `-f` param for "finished at" info for each pipeline
- added internal remaphore to prevent Gitlab flood
- code structured to modeles

### 0.1.1
- `-c` param for setting the number of pipelines on output
  (instead of fixed 3)
- pipeline total run time added

### 0.1.0
- initial release

## TODO
- error handling + comfy error outputs
- ~`-c` param for setting the number of pipelines on output
  (instead of fixed 3)~
