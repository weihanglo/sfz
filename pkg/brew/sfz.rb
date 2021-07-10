class Sfz < Formula
  version '0.6.1'
  desc "A simple static file serving command-line tool."
  homepage "https://github.com/weihanglo/sfz"
  head "https://github.com/weihanglo/sfz.git"

  if OS.mac?
    url "https://github.com/weihanglo/sfz/releases/download/v#{version}/sfz-v#{version}-x86_64-apple-darwin.tar.gz"
    sha256 "4641b7d247b31e79035197b6835acc4a0253406aa30a4387f0bf249ba06a05bc"
  elsif OS.linux?
    url "https://github.com/weihanglo/sfz/releases/download/v#{version}/sfz-v#{version}-x86_64-unknown-linux-musl.tar.gz"
    sha256 "9157c481e73da5501f8ce93527cf611c442d1dbae7a8114890787d53b1d07276"
  end

  def install
    bin.install "sfz"
  end

  test do
    system "#{bin}/sfz", "--help"
  end
end
