# Mailfilter

CLI for working with MBOX files.

## Usage

### Count

    mailfilter count inbox.mbox body=~/thank you/

Prints a count of messages that match the filter.

### Extract

    mailfilter extract inbox.mbox subject=~/thank you/

## Filters

Mailfilter has a mini query language for selecting individual letters, which is
loosely inspired by Lucene. For example:

    subject!~/re:/i and body=~/tax/

This will match any letter that does not match the regular expression /re:/i
(case insensitive) and that contains the text `tax` in the body.

Filters are made up of multiple match statements which begin with a field, have
an operator and then some value. The available operators are:

  * `=~` matches regular expression
  * `!~` does not match regular expression
  * `^~` starts with text
  * `$=` ends with text
  * `!=` does not match text
  * `=` matches literal text

Multiple match statements can be joined together with `and` or `or` statements.

## Shell and Filters

The filter program must be a single argument to mailfilter, so you'll often
have to surround in quotes, e.g:

    mailfilter extract inbox.mbox 'subject=~/thank you/ and body="AAA"'
