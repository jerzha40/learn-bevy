package com.example.conway;

import androidx.games.activity.GameActivity;

public class MainActivity extends GameActivity {
    static {
        System.loadLibrary("conway");
    }
}
