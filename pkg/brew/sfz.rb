class Sfz < Formula
  version '0.0.3'
  desc "A simple static file serving command-line tool."
  homepage "https://github.com/weihanglo/sfz"
  head "https://github.com/weihanglo/sfz.git"

  if OS.mac?
      url "https://github.com/weihanglo/sfz/releases/download/#{version}/sfz-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "34b10cd9c530f18ad252660b57dabad722faadbde5ad39ea1dc7fce279ad9c55"
  elsif OS.linux?
      url "https://github.com/weihanglo/sfz/releases/download/#{version}/sfz-#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "fdec9a1d1d5fa68816576a42ac70c714c5ed79598dcbe037d49d3cb644310a5e"
  end

  def install
    bin.install "sfz"
  end

  test do
    system "#{bin}/sfz", "--help"
  end
end
