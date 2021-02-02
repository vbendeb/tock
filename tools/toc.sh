#!/usr/bin/env bash

# Table of Contents for markdown files.
#
# Runs `md-toc` (from the [markdown-toc](https://crates.io/crates/markdown-toc)
# crate) on all Markdown files with tables of contents to see if any of them
# need an updated TOC.
#
# As a side effect, if you run this locally it will generate all needed TOC and
# update the markdown documents.
#
# To add a table of contents to a markdown file, add the tags:
#
#     <!-- toc -->
#     <!-- tocstop -->
#
# to the markdown file where you want the table of contents and run this script.
#
# Author: Brad Campbell <bradjc5@gmail.com>

let ERROR=0

# Make sure the `markdown-toc` tool is installed.
cargo install markdown-toc

# Find all markdown files
for f in $(find . -path ./node_modules -prune -false -o -name "*.md"); do

	# Only use ones that include a table of contents
	grep '<!-- toc -->' $f > /dev/null
	let rc=$?

	if [[ $rc == 0 ]]; then
		# Try running the TOC tool and see if anything changes
		before=`cat $f`
		tableofcontents=`md-toc $f --bullet "-" --indent 2 --no-header`
		replace='<!-- toc -->

<!-- Build table of contents with tools\/toc.sh -->
'$tableofcontents'

<!-- tocstop -->'
		perl -i -p0e "s/\<\!-- toc --\>.*\<\!-- tocstop --\>/$replace/gs" $f
		after=`cat $f`

		if [[ "$before" != "$after" ]]; then
			echo "$f has an outdated table of contents"
			ERROR=1
		fi
	fi

done

# Make sure to return with an error if anything changes
# so that CI will fail.
if [[ $ERROR == 1 ]]; then
	exit -1
fi
