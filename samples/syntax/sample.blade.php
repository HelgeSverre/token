{{-- Layout Template --}}
<!DOCTYPE html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}">
<head>
    <meta charset="utf-8">
    <meta name="csrf-token" content="{{ csrf_token() }}">
    <title>{{ config('app.name') }} - @yield('title', 'Home')</title>

    @vite(['resources/css/app.css', 'resources/js/app.js'])
    @livewireStyles
    @stack('styles')
</head>
<body class="antialiased">
    @include('partials.navigation')

    {{-- Flash Messages --}}
    @if (session('success'))
        <div class="alert alert-success">
            {{ session('success') }}
        </div>
    @elseif (session('error'))
        <div class="alert alert-danger">
            {{ session('error') }}
        </div>
    @endif

    <main class="container mx-auto px-4 py-8">
        @yield('content')
    </main>

    @section('footer')
        <footer class="bg-gray-800 text-white p-6">
            <p>&copy; {{ date('Y') }} {{ config('app.name') }}</p>
        </footer>
    @show

    @livewireScripts
    @stack('scripts')
</body>
</html>

{{-- ============================================================ --}}
{{-- Child View                                                    --}}
{{-- ============================================================ --}}

@extends('layouts.app')

@section('title', 'Dashboard')

@section('content')
    @auth
        <h1>Welcome back, {{ Auth::user()->name }}!</h1>

        @can('viewAdminPanel', App\Models\User::class)
            <div class="admin-panel bg-red-50 p-4 rounded-lg">
                @include('admin.quick-stats')
            </div>
        @endcan

        @cannot('viewAdminPanel')
            <p class="text-gray-600">Standard access.</p>
        @endcannot

        @feature('new-dashboard')
            <x-dashboard.new-layout :user="$user" />
        @else
            <x-dashboard.classic-layout :user="$user" />
        @endfeature
    @else
        <h1>Please <a href="{{ route('login') }}">log in</a>.</h1>
    @endauth

    @guest
        @include('partials.guest-welcome')
    @endguest

    {{-- Loops --}}
    @unless ($projects->isEmpty())
        <h2 class="text-xl font-semibold mb-4">Your Projects</h2>

        @foreach ($projects as $project)
            <div class="project-card p-4 border rounded-lg mb-3
                @class([
                    'bg-green-50' => $project->status === 'active',
                    'bg-yellow-50' => $project->status === 'pending',
                    'bg-gray-50' => $project->status === 'archived',
                ])">
                <h3>{{ $project->name }}</h3>
                <p>{{ Str::limit($project->description, 100) }}</p>
                <span class="text-sm text-gray-500">
                    Created {{ $project->created_at->diffForHumans() }}
                </span>

                @if ($loop->first)
                    <span class="badge badge-primary">Latest</span>
                @endif
            </div>
        @endforeach

        {{ $projects->links() }}
    @endunless

    @forelse ($notifications as $notification)
        <div class="notification {{ $notification->read ? 'opacity-50' : '' }}">
            <p>{!! $notification->message !!}</p>
            <small>{{ $notification->created_at->format('M d, Y H:i') }}</small>
        </div>
    @empty
        <p class="text-gray-400">No notifications yet.</p>
    @endforelse

    {{-- Switch --}}
    @switch($user->subscription)
        @case('free')
            <x-subscription.free-tier />
            @break
        @case('pro')
            <x-subscription.pro-tier :features="$proFeatures" />
            @break
        @default
            <p>Unknown tier.</p>
    @endswitch

    @for ($i = 0; $i < 5; $i++)
        <div class="step">Step {{ $i + 1 }}</div>
    @endfor

    @while ($item = array_pop($queue))
        <div>Processing: {{ $item }}</div>
    @endwhile

    @isset($featured)
        <x-featured-banner :item="$featured" />
    @endisset

    @empty($results)
        <x-empty-state message="No results found." />
    @endempty

    {{-- Form with error handling --}}
    <form method="POST" action="{{ route('profile.update') }}">
        @csrf
        @method('PATCH')

        <div class="form-group">
            <label for="name">Name</label>
            <input type="text" name="name" id="name"
                value="{{ old('name', $user->name) }}"
                @class(['form-control', 'is-invalid' => $errors->has('name')])
                @readonly($user->isLocked())
                @disabled(!$user->canEdit())
                @required>

            @error('name')
                <span class="text-red-500 text-sm">{{ $message }}</span>
            @enderror
        </div>

        <select name="country" @required>
            @foreach ($countries as $code => $name)
                <option value="{{ $code }}"
                    @selected(old('country', $user->country) === $code)>
                    {{ $name }}
                </option>
            @endforeach
        </select>

        <input type="checkbox" name="terms"
            @checked(old('terms', $user->accepted_terms))>

        <button type="submit">Update Profile</button>
    </form>

    {{-- Components --}}
    <x-card title="Statistics" class="mt-6">
        <x-slot:header>
            <h3 class="font-bold">Monthly Overview</h3>
        </x-slot:header>

        <x-stats-grid :stats="$monthlyStats" />

        <x-slot:footer>
            <a href="{{ route('stats.detail') }}">View Details &rarr;</a>
        </x-slot:footer>
    </x-card>

    <x-alert type="info" :dismissible="true">
        Your trial ends in {{ $daysLeft }} days.
    </x-alert>

    {{-- Props and Aware --}}
    @props(['color' => 'blue', 'size' => 'md'])
    @aware(['theme' => 'light'])

    {{-- Inline PHP --}}
    @php($total = $items->sum('price'))

    {{-- Multi-line PHP --}}
    @php
        $categories = $items->groupBy('category');
        $topCategory = $categories->sortByDesc(fn($g) => $g->count())->keys()->first();
        $averagePrice = $items->avg('price');
    @endphp

    {{-- Verbatim (for JS templating) --}}
    @verbatim
        <div id="vue-app">
            <h1>{{ title }}</h1>
            <p>{{ message }}</p>
        </div>
    @endverbatim

    {{-- Stacks --}}
    @push('scripts')
        <script src="{{ asset('js/dashboard.js') }}"></script>
    @endpush

    @pushOnce('scripts')
        <script src="{{ asset('js/chart.js') }}"></script>
    @endPushOnce

    @prepend('scripts')
        <script>window.config = @json($config);</script>
    @endprepend

    {{-- Fragment --}}
    @fragment('user-list')
        <ul id="user-list">
            @foreach ($users as $user)
                <li>{{ $user->name }}</li>
            @endforeach
        </ul>
    @endfragment

    {{-- Once --}}
    @once
        <style>
            .project-card { transition: all 0.2s ease; }
            .project-card:hover { transform: translateY(-2px); }
        </style>
    @endonce

    {{-- Environment checks --}}
    @production
        <script src="{{ mix('js/analytics.js') }}"></script>
    @endproduction

    @env('staging')
        <div class="staging-banner bg-yellow-400 p-2 text-center">
            Staging Environment
        </div>
    @endenv

    {{-- Include variants --}}
    @includeIf('partials.optional-sidebar')
    @includeWhen($user->isAdmin(), 'admin.toolbar')
    @includeUnless($user->isGuest(), 'partials.user-menu')
    @includeFirst(['custom.header', 'default.header'])

    @each('partials.project-card', $projects, 'project', 'partials.no-projects')

    @inject('metrics', 'App\Services\MetricsService')
    <p>Total users: {{ $metrics->getUserCount() }}</p>

    {{-- Raw PHP --}}
    <?php $debug = ['php' => PHP_VERSION, 'env' => app()->environment()]; ?>

    {{-- Unescaped output --}}
    {!! $page->rendered_html !!}

    {{-- Livewire --}}
    @livewire('search-users', ['role' => 'admin'])

    @persist('player')
        <audio id="player" src="{{ $podcast->url }}"></audio>
    @endpersist

    @teleport('#footer')
        <div class="modal" id="global-modal"></div>
    @endteleport

    @vite('resources/js/dashboard.js')
    @viteReactRefresh
@endsection

@push('styles')
    <link rel="stylesheet" href="{{ asset('css/dashboard.css') }}">
@endpush
