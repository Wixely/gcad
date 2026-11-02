cmake_minimum_required(VERSION 3.24)
foreach(required HELPER OUTPUT ASSIMP_SOURCE UFBX_SOURCE)
  if(NOT DEFINED ${required})
    message(FATAL_ERROR "Missing ${required}")
  endif()
endforeach()
get_filename_component(OUTPUT "${OUTPUT}" ABSOLUTE)
if(EXISTS "${OUTPUT}")
  message(FATAL_ERROR "Preserving existing helper package")
endif()
execute_process(COMMAND "${HELPER}" --handshake OUTPUT_VARIABLE identity
  RESULT_VARIABLE status ERROR_VARIABLE diagnostic)
if(NOT status EQUAL 0)
  message(FATAL_ERROR "Cannot inspect packaged helper")
endif()
string(JSON revision GET "${identity}" assimp_revision)
string(JSON patch GET "${identity}" assimp_diff_sha256)
if(NOT revision STREQUAL "a4643cbd30d3088bbff312d3cc2652f514fb97c0" OR
   NOT patch STREQUAL "783bf12bee4103422f8e8d2f74b2f17ae4dd93109b25266cd67baa9838340bf9")
  message(FATAL_ERROR "Unreviewed helper identity")
endif()
file(SHA256 "${CMAKE_CURRENT_LIST_DIR}/experimental-material-properties-v12.patch" patch_file_hash)
if(NOT patch_file_hash STREQUAL patch)
  message(FATAL_ERROR "Packaged patch does not match the helper identity")
endif()
file(MAKE_DIRECTORY "${OUTPUT}/licenses")
file(COPY "${HELPER}" DESTINATION "${OUTPUT}")
file(WRITE "${OUTPUT}/helper-identity.json" "${identity}")
file(COPY "${CMAKE_CURRENT_LIST_DIR}/HELPER-BUNDLE.md" DESTINATION "${OUTPUT}")
file(COPY_FILE "${CMAKE_CURRENT_LIST_DIR}/../../LICENSE" "${OUTPUT}/licenses/gcad.txt")
file(COPY_FILE "${ASSIMP_SOURCE}/LICENSE" "${OUTPUT}/licenses/assimp.txt")
file(COPY_FILE "${UFBX_SOURCE}/LICENSE" "${OUTPUT}/licenses/ufbx.txt")
foreach(item zlib/LICENSE rapidjson/license.txt poly2tri/LICENSE openddlparser/LICENSE
    pugixml/LICENSE.md utf8cpp/doc/LICENSE earcut-hpp/LICENSE
    stb/stb_image.h unzip/unzip.h unzip/ioapi.h unzip/crypt.h clipper/License.txt
    Open3DGC/o3dgcArithmeticCodec.cpp Open3DGC/o3dgcCommon.h)
  string(REPLACE "/" "-" name "${item}")
  file(COPY_FILE "${ASSIMP_SOURCE}/contrib/${item}" "${OUTPUT}/licenses/${name}")
endforeach()
file(COPY "${CMAKE_CURRENT_LIST_DIR}/experimental-material-properties-v12.patch" DESTINATION "${OUTPUT}")
# Export only the pinned public dependency commit, without checkout-local
# configuration, remotes, hooks, reflogs, or untracked files. Git object storage
# preserves source filenames and modes and is directly rebuildable offline.
function(snapshot_dependency source revision name)
  set(repository "${OUTPUT}/source/${name}.git")
  execute_process(COMMAND git init --bare --template= "${repository}"
    OUTPUT_QUIET COMMAND_ERROR_IS_FATAL ANY)
  execute_process(COMMAND git -C "${repository}" config core.logAllRefUpdates false
    COMMAND_ERROR_IS_FATAL ANY)
  execute_process(COMMAND git -C "${repository}" fetch --depth=1 --no-tags "${source}" "${revision}"
    OUTPUT_QUIET COMMAND_ERROR_IS_FATAL ANY)
  execute_process(COMMAND git -C "${repository}" update-ref refs/heads/source "${revision}"
    COMMAND_ERROR_IS_FATAL ANY)
  execute_process(COMMAND git -C "${repository}" symbolic-ref HEAD refs/heads/source
    COMMAND_ERROR_IS_FATAL ANY)
  # FETCH_HEAD contains the local checkout path and is not needed to clone.
  file(REMOVE "${repository}/FETCH_HEAD")
endfunction()
snapshot_dependency("${ASSIMP_SOURCE}" "${revision}" assimp)
snapshot_dependency("${UFBX_SOURCE}" "5955c5c0b042ac2dc6f32955ab206aaec0620cfc" ufbx)
file(GLOB helper_sources "${CMAKE_CURRENT_LIST_DIR}/*.h" "${CMAKE_CURRENT_LIST_DIR}/*.cpp"
  "${CMAKE_CURRENT_LIST_DIR}/*.cmake" "${CMAKE_CURRENT_LIST_DIR}/CMakeLists.txt")
file(COPY ${helper_sources} DESTINATION "${OUTPUT}/source/helper")
file(COPY "${CMAKE_CURRENT_LIST_DIR}/rebuild-helper.sh" "${CMAKE_CURRENT_LIST_DIR}/SOURCE-BUILD.md"
  DESTINATION "${OUTPUT}")
file(GLOB_RECURSE files RELATIVE "${OUTPUT}" "${OUTPUT}/*")
if(NOT files)
  message(FATAL_ERROR "Package file inventory is empty")
endif()
list(SORT files)
set(checksums "")
foreach(file IN LISTS files)
  file(SHA256 "${OUTPUT}/${file}" hash)
  string(APPEND checksums "${hash}  ${file}\n")
endforeach()
string(STRIP "${checksums}" checksums)
file(CONFIGURE OUTPUT "${OUTPUT}/SHA256SUMS" CONTENT "@checksums@" @ONLY NEWLINE_STYLE UNIX)
message(STATUS "Experimental helper bundle created; engine acceptance remains incomplete")
