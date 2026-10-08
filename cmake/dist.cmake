# SPDX-License-Identifier: Apache-2.0
#
# Copies the built program to <DEST_DIR>/<NAME> and records its SHA-256 in <DEST_DIR>/SHA256SUMS.
# Run by the `dist` target of CMakeLists.txt:
#   cmake -DSRC=<built program> -DDEST_DIR=<folder> -DNAME=<file name> -P cmake/dist.cmake
#
# The sums file keeps the lines of other files (other systems built into the same folder): the
# line of NAME is replaced, the others stay, the lines are sorted by file name.

foreach(_required SRC DEST_DIR NAME)
  if(NOT DEFINED ${_required} OR "${${_required}}" STREQUAL "")
    message(FATAL_ERROR "dist.cmake: -D${_required}=... is required")
  endif()
endforeach()
if(NOT EXISTS "${SRC}")
  message(FATAL_ERROR "dist.cmake: the program ${SRC} was not built")
endif()

file(MAKE_DIRECTORY "${DEST_DIR}")
file(COPY_FILE "${SRC}" "${DEST_DIR}/${NAME}" ONLY_IF_DIFFERENT)
file(SHA256 "${DEST_DIR}/${NAME}" _hash)

set(_sums_file "${DEST_DIR}/SHA256SUMS")
set(_lines "")
if(EXISTS "${_sums_file}")
  file(STRINGS "${_sums_file}" _old_lines)
  foreach(_line IN LISTS _old_lines)
    # "<hash>  <name>": the line of NAME is dropped (compared as text, not as a pattern, so a
    # `+` or a dot in the name is no problem), the others stay
    string(REGEX REPLACE "^[0-9a-fA-F]+  " "" _old_name "${_line}")
    if(NOT _old_name STREQUAL "${NAME}" AND NOT _line STREQUAL "")
      list(APPEND _lines "${_line}")
    endif()
  endforeach()
endif()
list(APPEND _lines "${_hash}  ${NAME}")
# sort by the file name (the text after the two spaces)
set(_keyed "")
foreach(_line IN LISTS _lines)
  string(REGEX REPLACE "^[0-9a-f]+  " "" _file "${_line}")
  list(APPEND _keyed "${_file}|${_line}")
endforeach()
list(SORT _keyed)
set(_out "")
foreach(_entry IN LISTS _keyed)
  string(REGEX REPLACE "^[^|]*\\|" "" _line "${_entry}")
  string(APPEND _out "${_line}\n")
endforeach()
file(WRITE "${_sums_file}" "${_out}")

message(STATUS "dist: ${DEST_DIR}/${NAME}")
message(STATUS "dist: sha256 ${_hash}")
