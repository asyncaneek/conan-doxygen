# conan-doxygen
This repo contains documentation for CarteNav Development packages

## Pre-requisites
doxygen - download and install from https://github.com/doxygen/doxygen/releases
cargo - rust tools
conan - C++ package manager

## Build
This is cli application written in Rust. To build:
`cd ./cn-doxy`
`cargo build`

Usage: conan-doxygen [OPTIONS] <SRC>

Arguments:
  <SRC>       Path to conan package

Options:
  --out <OUT>  Path to output folder
  --open       Open generated documentation
  -h, --help   Print help

## Output
The tool does the following steps:
- run conan inspect to find the name and version of the package
- run conan install to fetch the dependencies
- run conan info to find and compile a list of sources of the dependencies from the cache
- append source files for the parent package to the list of sources
- generate a DoxyFile configuration using a template, filling in the properties for sources and output
- run doxygen to generate docs for all the packages
- open the ./index.html in default browser

## Notes:
Doxygen Awesome CSS is use to style the html output
ref: https://github.com/jothepro/doxygen-awesome-css
todo: add dark/light theme switcher
