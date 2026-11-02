# Refuse source drift between configuration and compilation of the helper.
execute_process(COMMAND git -C "${ASSIMP_SOURCE}" rev-parse HEAD
  OUTPUT_VARIABLE revision OUTPUT_STRIP_TRAILING_WHITESPACE COMMAND_ERROR_IS_FATAL ANY)
execute_process(COMMAND git -C "${ASSIMP_SOURCE}" diff --no-ext-diff --binary --full-index HEAD --
  OUTPUT_VARIABLE diff COMMAND_ERROR_IS_FATAL ANY)
string(SHA256 diff_hash "${diff}")
if(NOT revision STREQUAL EXPECTED_REVISION OR NOT diff_hash STREQUAL EXPECTED_DIFF)
  message(FATAL_ERROR "Assimp source changed after configuration; reconfigure and review helper identity")
endif()
