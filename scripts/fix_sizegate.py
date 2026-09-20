#!/usr/bin/env python3
"""Fix the force-unwrap regex in ci-size-gate.sh (audit C14).

The bug: the bracket expression [A-Za-z0-9_)\\]] closes early at \\], so `]` is treated as a required literal. This means `x!` is never matched, only patterns ending with characters in the class.

The fix per the audit: replace with `(try!|[]A-Za-z0-9_)]!([^=]|$))` - a properly formed bracket class where `]` is the first character.
"""
import re

path = "scripts/ci-size-gate.sh"
with open(path, "r") as f:
    content = f.read()

# The regex as it appears in the file. The shell passes to grep:
#   '(try!|[A-Za-z0-9_)\\]]\\s*!(=|\\?|;|,|\\)|\\s|$))'
# In the file, the single-quoted string is literal. The backslash before ] and the double backslashes are shell/grep escaping.
# We need to match the exact bytes in the file.

# Let me just find it with regex
old_pattern = re.compile(r"\(try!\|\[A-Za-z0-9_\)\\\\\]\]\\\\s\*!\(=\\\\?\|;\|,\|\\\\)\\|\\\\s\|\$\)\)")
new_str = "(try!|[]A-Za-z0-9_)]!([^=]|$))"

matches = list(old_pattern.finditer(content))
print(f"Found {len(matches)} matches with regex")

# Try simpler approach - find lines containing the pattern and replace inline
lines = content.split('\n')
new_lines = []
replaced = 0
for line in lines:
    if 'A-Za-z0-9_)' in line and 'try!' in line:
        # Replace the exact substring
        # The actual text in the file is: (try!|[A-Za-z0-9_)\\]]\\s*!(=|\\?|;|,|\\)|\\s|$))
        # Let me find where this substring is
        idx = line.find('(try!|')
        if idx != -1:
            # Find the matching closing paren
            depth = 0
            end = idx
            for i in range(idx, len(line)):
                if line[i] == '(':
                    depth += 1
                elif line[i] == ')':
                    depth -= 1
                    if depth == 0:
                        end = i + 1
                        break
            old_sub = line[idx:end]
            print(f"Old substring: {repr(old_sub)}")
            # Construct replacement keeping the same overall structure
            # Replace the bracket expression part
            new_sub = old_sub.replace('[A-Za-z0-9_)\\\\]]', '[]A-Za-z0-9_)]')
            new_sub = new_sub.replace('\\s*!(=|\\\\?|;|,|\\\\)|\\\\s|$)', '!([^=]|$)')
            new_lines.append(line[:idx] + new_sub + line[end:])
            replaced += 1
        else:
            new_lines.append(line)
    else:
        new_lines.append(line)

print(f"Replaced {replaced} lines")
with open(path, "w") as f:
    f.write('\n'.join(new_lines))
