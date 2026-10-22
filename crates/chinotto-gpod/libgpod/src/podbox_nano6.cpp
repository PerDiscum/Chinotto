#include "podbox/itdb/hashab.h"
#include "podbox/itdb/itunesdb.h"
#include "podbox/itdb/itunessqlite.h"

#include <algorithm>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>
#include <unordered_map>
#include <vector>

struct Nano6Playlist {
    std::uint64_t dbid;
    const char* name;
    const std::uint64_t* trackDbids;
    std::size_t trackCount;
    int isMaster;
    int isSmart;
};

namespace {

void report(char* out, std::size_t capacity, const std::string& message)
{
    if (out == nullptr || capacity == 0) return;
    std::snprintf(out, capacity, "%s", message.c_str());
}

bool parseGuid(const char* text, std::vector<std::uint8_t>& uuid)
{
    if (text == nullptr || std::strlen(text) != 16) return false;
    uuid.clear();
    for (std::size_t i = 0; i < 16; i += 2) {
        unsigned int byte = 0;
        if (std::sscanf(text + i, "%2x", &byte) != 1) return false;
        uuid.push_back(static_cast<std::uint8_t>(byte));
    }
    return true;
}

} // namespace

extern "C" int chinotto_nano6_stage(const char* rootText,
                                     const char* cdbStageText,
                                     const char* sqliteStageText,
                                     const char* guid,
                                     const Nano6Playlist* playlists,
                                     std::size_t playlistCount,
                                     char* error,
                                     std::size_t errorCapacity)
{
    namespace fs = std::filesystem;
    if (rootText == nullptr || cdbStageText == nullptr || sqliteStageText == nullptr ||
        (playlistCount != 0 && playlists == nullptr)) {
        report(error, errorCapacity, "Missing Nano 6 path or playlist data");
        return 0;
    }

    const fs::path root(rootText);
    const fs::path cdb = root / "iPod_Control/iTunes/iTunesCDB";
    const fs::path libraryDir = root / "iPod_Control/iTunes/iTunes Library.itlp";
    std::vector<std::uint8_t> uuid;
    if (!parseGuid(guid, uuid)) {
        report(error, errorCapacity, "The Nano 6 USB identifier is missing or invalid");
        return 0;
    }

    podbox::ParseResult parsed = podbox::parseItunesDb(cdb);
    if (!parsed.library) {
        report(error, errorCapacity, "Could not parse the Nano 6 database: " + parsed.error);
        return 0;
    }
    podbox::Library& library = *parsed.library;
    if (!library.compressed || library.hashingScheme != podbox::kChecksumHashAB) {
        report(error, errorCapacity, "The Nano 6 database is not a compressed HashAB database");
        return 0;
    }

    std::ifstream input(cdb, std::ios::binary);
    std::vector<std::uint8_t> bytes(std::istreambuf_iterator<char>(input), {});
    const auto nonce = podbox::hashAbExtractNonce(podbox::storedHashAb(bytes),
                                                   podbox::hashAbSha1(bytes), uuid);
    if (!nonce) {
        report(error, errorCapacity, "Nano 6 HashAB verification failed; refusing to write");
        return 0;
    }

    std::unordered_map<std::uint64_t, std::uint32_t> idByDbid;
    for (const podbox::Track& track : library.tracks) idByDbid[track.dbid] = track.id;
    std::vector<podbox::Playlist> changed;
    changed.reserve(playlistCount);
    for (std::size_t i = 0; i < playlistCount; ++i) {
        const Nano6Playlist& in = playlists[i];
        if (in.isMaster) {
            if (in.name != nullptr) library.masterName = in.name;
            library.masterDbid = in.dbid;
            continue;
        }
        auto old = std::find_if(library.playlists.begin(), library.playlists.end(),
                                [&](const podbox::Playlist& p) { return p.dbid == in.dbid; });
        podbox::Playlist out;
        if (old != library.playlists.end()) out = std::move(*old);
        out.dbid = in.dbid;
        out.name = in.name == nullptr ? "" : in.name;
        if (!in.isSmart) {
            out.trackIds.clear();
            for (std::size_t n = 0; n < in.trackCount; ++n) {
                const auto track = idByDbid.find(in.trackDbids[n]);
                if (track == idByDbid.end()) {
                    report(error, errorCapacity,
                           "A playlist refers to a song missing from the Nano 6 database; refusing to write");
                    return 0;
                }
                out.trackIds.push_back(track->second);
            }
        }
        changed.push_back(std::move(out));
    }
    library.playlists = std::move(changed);

    podbox::WriteOptions options;
    options.hashAbUuid = uuid;
    options.hashAbNonce = *nonce;
    options.compressed = true;
    std::string writeError;
    const fs::path stagedCdb(cdbStageText);
    if (!podbox::writeItunesDb(library, stagedCdb, &writeError, options)) {
        report(error, errorCapacity, "Could not prepare Nano 6 iTunesCDB: " + writeError);
        return 0;
    }

    podbox::ParseResult staged = podbox::parseItunesDb(stagedCdb);
    if (!staged.library || !staged.library->compressed ||
        staged.library->hashingScheme != podbox::kChecksumHashAB) {
        report(error, errorCapacity, "Staged Nano 6 iTunesCDB failed its format validation");
        return 0;
    }
    std::ifstream stagedInput(stagedCdb, std::ios::binary);
    std::vector<std::uint8_t> stagedBytes(std::istreambuf_iterator<char>(stagedInput), {});
    const auto stagedNonce = podbox::hashAbExtractNonce(
        podbox::storedHashAb(stagedBytes), podbox::hashAbSha1(stagedBytes), uuid);
    if (!stagedNonce || *stagedNonce != *nonce) {
        report(error, errorCapacity, "Staged Nano 6 iTunesCDB HashAB verification failed");
        return 0;
    }

    if (!podbox::writeItunesSqliteBundle(*staged.library, libraryDir,
                                         fs::path(sqliteStageText), uuid, *nonce,
                                         &writeError)) {
        report(error, errorCapacity, "Could not prepare Nano 6 companion databases: " + writeError);
        return 0;
    }
    report(error, errorCapacity, "");
    return 1;
}
