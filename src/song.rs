use lazy_static::lazy_static;
use log::error;
use regex::Regex;

use crate::utils::to_greeklish;

/// Remove lines that contain only chords and symbols and trim unneeded characters.
pub fn strip_metadata(string: &str) -> String {
    // We are interested in Greek lyrics so we can skip every line that only contains ASCII.
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^[[:ascii:]]*$").unwrap();
    }

    let lines: Vec<&str> = string
        .split('\n')
        .filter(|line| !RE.is_match(line))
        .map(|line| {
            // Trim any symbols that indicate lyric repetition (e.g. "| 2x")
            line.trim_end_matches(|c: char| c == ' ' || c == '|' || c.is_ascii_digit() || c == 'x')
        })
        .collect();

    // Remove dashes that represent syllable repetition
    lines.join("\n").chars().filter(|&c| c != '-').collect()
}

pub struct Song {
    pub name: String,
    pub slug: String,
    pub artist: String,
    pub artist_slug: String,
    pub body: String,
    pub body_greeklish: String,
}

impl Song {
    pub fn from_path(path: &std::path::PathBuf) -> tantivy::Result<Song> {
        let contents = std::fs::read_to_string(path.clone())?;
        let mut parts = contents.splitn(4, "\n\n");
        let head = parts.next().unwrap();
        let _song_scale = parts.next().unwrap();
        let _song_rhythm = parts.next().unwrap();
        let song_body = parts.next().unwrap();

        let head_parts: Vec<&str> = head.split('\n').collect();
        let (song_name, song_artist) = match head_parts[..] {
            [song_name, song_artist, _song_url] => (song_name, song_artist),
            [song_name, song_artist] => (song_name, song_artist),
            _ => {
                error!("Invalid song format");
                std::process::exit(1);
            }
        };

        let song_body = strip_metadata(song_body);
        let song_body_greeklish = to_greeklish(song_body.as_str());

        Ok(Song {
            name: String::from(song_name),
            slug: to_greeklish(song_name),
            artist: String::from(song_artist),
            artist_slug: to_greeklish(song_artist),
            body: song_body,
            body_greeklish: song_body_greeklish,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use crate::song::Song;

    #[test]
    fn test_song() {
        let mut file = NamedTempFile::new().unwrap();
        let file_content = concat!(
            "Τα μπλε παράθυρά σου\n",
            "Μάρκος Βαμβακάρης\n",
            "https://www.youtube.com/watch?v=CPYwCdRL8GU\n",
            "\n",
            "B  Φυσικό Μινόρε\n",
            "\n",
            "Ζεϊμπέκικο Παλιό\n",
            "\n",
            "Bm  Bm  F#  Bm   | 4x\n",
            "\n",
            "D\n",
            "Περνούσα και σ' αντίκρυζα ψηλά στα παραθύρια   | 2x\n",
            "Em\n",
            "και τότες πια καμάρωνα τα δυο σου μαύρα φρύδια\n",
            "Em                        F#            Bm\n",
            "και τότες πια καμάρωνα τα δυο σου μαύρα φρύδια\n",
            "\n",
            "Επήγες σ' άλλη γειτονιά κι εγώ τρελός γυρίζω\n",
            "με παίρνει το παράπονο κι ανώφελα δακρύζω\n",
            "\n",
            "Πού να γυρίσω να σε βρω στη γη στην οικουμένη\n",
            "γιατί έφυγες και μ' άφησες με την καρδιά καμένη\n",
            "\n",
            "Ξενοίκιασε το σπίτι σου κι έλα στη γειτονιά σου\n",
            "όπως και πριν να σε θωρώ απ' τα παράθυρά σου\n",
        );
        file.write_all(file_content.as_bytes()).unwrap();

        let song = Song::from_path(&file.path().to_path_buf()).unwrap();

        assert_eq!(song.name, "Τα μπλε παράθυρά σου");
        assert_eq!(song.slug, "ta_mple_parathyra_sou");
        assert_eq!(song.artist, "Μάρκος Βαμβακάρης");
        assert_eq!(song.artist_slug, "markos_vamvakaris");
        assert_eq!(
            song.body,
            concat!(
                "Περνούσα και σ' αντίκρυζα ψηλά στα παραθύρια\n",
                "και τότες πια καμάρωνα τα δυο σου μαύρα φρύδια\n",
                "και τότες πια καμάρωνα τα δυο σου μαύρα φρύδια\n",
                "Επήγες σ' άλλη γειτονιά κι εγώ τρελός γυρίζω\n",
                "με παίρνει το παράπονο κι ανώφελα δακρύζω\n",
                "Πού να γυρίσω να σε βρω στη γη στην οικουμένη\n",
                "γιατί έφυγες και μ' άφησες με την καρδιά καμένη\n",
                "Ξενοίκιασε το σπίτι σου κι έλα στη γειτονιά σου\n",
                "όπως και πριν να σε θωρώ απ' τα παράθυρά σου",
            )
        );
        assert_eq!(
            song.body_greeklish,
            concat!(
                "pernousa_kai_s_antikryza_psila_sta_parathyria\n",
                "kai_totes_pia_kamarona_ta_dyo_sou_maura_frydia\n",
                "kai_totes_pia_kamarona_ta_dyo_sou_maura_frydia\n",
                "epiges_s_alli_geitonia_ki_ego_trelos_gyrizo\n",
                "me_pairnei_to_parapono_ki_anofela_dakryzo\n",
                "pou_na_gyriso_na_se_vro_sti_gi_stin_oikoumeni\n",
                "giati_efyges_kai_m_afises_me_tin_kardia_kameni\n",
                "ksenoikiase_to_spiti_sou_ki_ela_sti_geitonia_sou\n",
                "opos_kai_prin_na_se_thoro_ap_ta_parathyra_sou",
            )
        );
    }
}
