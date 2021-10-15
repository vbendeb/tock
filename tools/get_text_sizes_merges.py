#!/usr/bin/env python3


MERGES_GIT_CMD = "log --merges --oneline HEAD~{}..HEAD"

OUTFILE = "sizes.data"

from sh import make
from sh import git


with open(OUTFILE, "w") as f:
    f.write("commit_hash	pr_number	text_size	data_size	bss_size\n")


commits = git(MERGES_GIT_CMD.format(50).split(), _tty_out=False)


print(commits)


for commit in commits:
    # Each commit looks like:
    #   33e464c7d Merge #2821
    #   5cd230d09 Merge pull request #2869 from saki-osive/master
    fields = commit.split()
    # Commit hash comes first
    commit_hash = fields[0]
    # Extract PR number
    pr_number = ""
    for field in fields[1:]:
        if field[0] == "#":
            pr_number = field

    # Checkout commit
    git.checkout(commit_hash)

    print("on hash  {}".format(commit_hash))

    make_output = make()

    text_size = 0
    data_size = 0
    bss_size = 0
    next_line_is_sizes = False
    for make_output_line in make_output:
        fields = make_output_line.split()
        if next_line_is_sizes == True:
            # Last line we found "text", so the first item on this line is the
            # size of the text section.
            text_size = int(fields[0])
            data_size = int(fields[1])
            bss_size = int(fields[2])
            break
        if fields[0] == "text":
            next_line_is_sizes = True

    display = "{}\t{}\t{}\t{}\t{}".format(
        commit_hash, pr_number, text_size, data_size, bss_size
    )
    print(display)

    with open(OUTFILE, "a") as f:
        f.write(display)
        f.write("\n")

# Go back to master, I guess
git.checkout("master")
